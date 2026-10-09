import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { IconChevronRight as Prompt, IconEraser as Eraser, IconLoader2 as Loader } from "@tabler/icons-react";
import { getConsoleWelcome, runConsoleCommand, SharedApiError } from "../lib/sharedApi";
import type { ConsoleResponse, ConsoleWelcome } from "../types";
import { Button } from "./ui/button";
import { showToast } from "./ui/toast";

const HISTORY_KEY = "repomemo.system.console-history";
const HISTORY_LIMIT = 100;

/** Lines worth trying first, shown as shortcuts under the prompt. */
const SUGGESTIONS = ["help", "status", "jobs list --status failed", "logs --level warn --limit 20", "maintenance status"];

interface Entry {
  id: number;
  line: string;
  /** `running` until the answer arrives; `error` for a refused command; `not_run` when the server was not reached. */
  state: "running" | "done" | "error" | "not_run";
  response?: ConsoleResponse;
  error?: string;
  /** `--json` was on the line: show the raw answer. */
  raw: boolean;
}

function readHistory(): string[] {
  try {
    const stored = JSON.parse(window.localStorage.getItem(HISTORY_KEY) ?? "[]") as unknown;
    return Array.isArray(stored) ? stored.filter((line): line is string => typeof line === "string").slice(-HISTORY_LIMIT) : [];
  } catch { return []; }
}

function writeHistory(history: string[]) {
  try { window.localStorage.setItem(HISTORY_KEY, JSON.stringify(history.slice(-HISTORY_LIMIT))); } catch { /* history is a convenience only */ }
}

/** Removes the client-side `--json` option; the server never sees it. */
function stripJsonFlag(line: string): { command: string; raw: boolean } {
  const parts = line.split(/\s+/);
  const raw = parts.includes("--json");
  return { command: raw ? parts.filter((part) => part !== "--json").join(" ") : line, raw };
}

function cell(value: unknown): string {
  if (value === null || value === undefined || value === "") return "—";
  if (typeof value === "boolean") return value ? "yes" : "no";
  if (typeof value === "number") return value.toLocaleString();
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function Answer({ entry }: { entry: Entry }) {
  const { response } = entry;
  if (entry.state === "running") return <p className="rm-console-muted"><Loader className="spin" size={14} /> Running…</p>;
  if (entry.state === "not_run") return <p className="rm-console-muted">Not run.</p>;
  if (entry.state === "error" || !response) return <p className="rm-console-error">{entry.error}</p>;
  if (entry.raw) return <pre className="rm-console-json">{JSON.stringify(response, null, 2)}</pre>;
  const { view, data } = response;
  return <>
    <p className={response.dry_run ? "rm-console-dry-run" : "rm-console-summary"}>{response.dry_run ? <span className="rm-console-tag">dry run</span> : null}{response.summary}</p>
    {view.kind === "facts" && Array.isArray(data) ? <dl className="rm-system-facts rm-console-facts">
      {(data as [string, unknown][]).map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{cell(value)}</dd></div>)}
    </dl> : null}
    {view.kind === "table" && Array.isArray(data) && data.length ? <div className="rm-map-coverage rm-console-table"><table>
      <thead><tr>{view.columns.map((column) => <th key={column} scope="col">{column.replace(/_/g, " ")}</th>)}</tr></thead>
      <tbody>{(data as Record<string, unknown>[]).map((row, index) => <tr key={index}>
        {view.columns.map((column) => <td key={column}>{cell(row[column])}</td>)}
      </tr>)}</tbody>
    </table></div> : null}
  </>;
}

/**
 * System › Console: a command line to the server for technical administrators, the same as the
 * `repomemo-console` terminal client (same logo, same commands). Each line goes to
 * `POST /v1/system/console`, which runs the same handlers as the System pages; changes are dry runs
 * until the line carries `--yes`. Up/Down walk the history, Tab completes, Ctrl+L clears.
 */
export function SystemConsole({ accessToken }: { accessToken: string }) {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [input, setInput] = useState("");
  const [welcome, setWelcome] = useState<ConsoleWelcome | null>(null);
  const commands = useMemo(() => welcome?.commands ?? [], [welcome]);
  const [history, setHistory] = useState<string[]>(readHistory);
  /** Position while walking the history; `null` when editing a new line. */
  const [historyIndex, setHistoryIndex] = useState<number | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const nextId = useRef(1);
  const inputRef = useRef<HTMLInputElement>(null);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getConsoleWelcome(accessToken).then(setWelcome).catch(() => showToast("error", "The console could not be opened."));
  }, [accessToken]);
  useEffect(() => { endRef.current?.scrollIntoView({ block: "end" }); }, [entries]);

  /** The usage of the command being typed, or the commands it could become. */
  const hint = useMemo(() => {
    const typed = input.trim().toLowerCase().replace(/\s+/g, " ");
    if (!typed) return "";
    const exact = commands.filter((command) => typed === command.name || typed.startsWith(`${command.name} `)).sort((a, b) => b.name.length - a.name.length)[0];
    if (exact) return exact.usage;
    const candidates = commands.filter((command) => command.name.startsWith(typed));
    return candidates.length ? candidates.map((command) => command.name).join(" · ") : "";
  }, [commands, input]);

  function remember(line: string) {
    const next = [...history.filter((previous) => previous !== line), line].slice(-HISTORY_LIMIT);
    setHistory(next);
    writeHistory(next);
    setHistoryIndex(null);
  }

  async function submit(line: string) {
    const trimmed = line.trim();
    if (!trimmed || isRunning) return;
    setInput("");
    remember(trimmed);
    if (trimmed === "clear" || trimmed === "cls") { setEntries([]); return; }
    const { command, raw } = stripJsonFlag(trimmed);
    const id = nextId.current++;
    setEntries((current) => [...current, { id, line: trimmed, state: "running", raw }]);
    setIsRunning(true);
    const settle = (patch: Partial<Entry>) => setEntries((current) => current.map((entry) => entry.id === id ? { ...entry, ...patch } : entry));
    try {
      settle({ state: "done", response: await runConsoleCommand(accessToken, command) });
    } catch (error) {
      // A refused command is console output; a server that cannot answer is a notification.
      if (error instanceof SharedApiError && error.status >= 400 && error.status < 500 && error.status !== 401) {
        settle({ state: "error", error: error.message });
      } else {
        settle({ state: "not_run" });
        showToast("error", error instanceof Error && error.message ? error.message : "The server could not run this command.");
      }
    } finally {
      setIsRunning(false);
      inputRef.current?.focus();
    }
  }

  function complete() {
    const typed = input.trimStart().toLowerCase().replace(/\s+/g, " ");
    const candidates = commands.map((command) => command.name).filter((name) => name.startsWith(typed) && name !== typed.trim());
    if (!candidates.length) return;
    // The longest prefix every candidate shares, plus a space once it names one command.
    let shared = candidates[0];
    for (const name of candidates) { while (!name.startsWith(shared)) shared = shared.slice(0, -1); }
    setInput(candidates.length === 1 ? `${shared} ` : shared);
  }

  function walkHistory(step: -1 | 1) {
    if (!history.length) return;
    const from = historyIndex ?? history.length;
    const to = Math.min(history.length, Math.max(0, from + step));
    setHistoryIndex(to === history.length ? null : to);
    setInput(to === history.length ? "" : history[to]);
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter") { event.preventDefault(); void submit(input); }
    else if (event.key === "ArrowUp") { event.preventDefault(); walkHistory(-1); }
    else if (event.key === "ArrowDown") { event.preventDefault(); walkHistory(1); }
    else if (event.key === "Tab" && input.trim()) { event.preventDefault(); complete(); }
    else if (event.key === "Escape") { setInput(""); setHistoryIndex(null); }
    else if (event.key.toLowerCase() === "l" && event.ctrlKey) { event.preventDefault(); setEntries([]); }
  }

  return <section className="shared-settings-group rm-console">
    <div className="rm-system-toolbar">
      <span className="shared-muted-copy">Commands run on the server with your rights. Changes are dry runs until you add <code>--yes</code>; add <code>--json</code> to see the raw answer.</span>
      <Button disabled={!entries.length} onClick={() => setEntries([])} type="button" variant="secondary"><Eraser size={16} /> Clear</Button>
    </div>
    <div aria-label="Console output" aria-live="polite" className="rm-console-screen" onClick={() => inputRef.current?.focus()} role="log">
      {welcome ? <div className="rm-console-welcome">
        <pre aria-label="RepoMemo" className="rm-console-banner" role="img">{welcome.banner}</pre>
        <p className="rm-console-muted">{welcome.tagline}</p>
        <p>Signed in as <strong>{welcome.user}</strong> ({welcome.role}) · RepoMemo {welcome.version}</p>
        <p className="rm-console-muted">{welcome.tips} Up and Down recall earlier lines, Tab completes, Ctrl+L clears.</p>
      </div> : <p className="rm-console-muted"><Loader className="spin" size={14} /> Opening the console…</p>}
      {entries.map((entry) => <div className="rm-console-entry" key={entry.id}>
        <p className="rm-console-line"><Prompt aria-hidden size={14} />{entry.line}</p>
        <Answer entry={entry} />
      </div>)}
      <div ref={endRef} />
    </div>
    <div className="rm-console-prompt">
      <Prompt aria-hidden size={16} />
      <input
        aria-describedby="rm-console-hint"
        aria-label="Console command"
        autoCapitalize="off"
        autoComplete="off"
        autoCorrect="off"
        autoFocus
        className="rm-console-input"
        onChange={(event) => { setInput(event.target.value); setHistoryIndex(null); }}
        onKeyDown={onKeyDown}
        placeholder="status"
        ref={inputRef}
        spellCheck={false}
        value={input}
      />
      {isRunning ? <Loader aria-label="Running" className="spin" size={16} /> : null}
    </div>
    <p className="rm-console-hint" id="rm-console-hint">{hint || " "}</p>
    <div className="rm-console-suggestions">
      {SUGGESTIONS.map((line) => <button className="rm-console-suggestion" key={line} onClick={() => { setInput(line); inputRef.current?.focus(); }} type="button">{line}</button>)}
    </div>
  </section>;
}
