import { useState } from "react";
import { IconLayoutList as LayoutList } from "@tabler/icons-react";
import { downloadSharedWorkspaceTemplateFile, saveSharedWorkspaceTemplate } from "../lib/sharedApi";
import { saveBlob } from "../lib/saveBlob";
import type { WorkspaceTemplateView } from "../types";
import { ADMIN_WORKSPACE_SECTIONS, WORKSPACE_SECTIONS, type WorkspaceSection } from "../web/constants";
import { apiMessage } from "../web/format";
import { adjustmentsFrom, previewLines, questionsFromText, SECTION_NAMES } from "../web/templatePreview";
import { useTemplate, useTemplateCatalogue, useTemplateDetail } from "../web/workspace/data";
import { TemplatePicker } from "./TemplatePicker";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import { useConfirm } from "./ui/confirm-dialog";
import { Dropdown } from "./ui/dropdown";
import { Input } from "./ui/input";
import { PanelHeading } from "./ui/panel-heading";
import { Textarea } from "./ui/textarea";
import { showToast } from "./ui/toast";

const MAX_QUESTIONS = 6;

/**
 * Settings › Template (V0.2.177), for owners and administrators: which template the workspace uses, a preview of what
 * another would make of it, the few adjustments that can be saved on top (the section it opens on, what sits under
 * More, the suggested questions) and a reset to the template's own values. Choosing or adjusting a template never
 * moves, creates or deletes evidence, and it changes nothing about who may do what.
 */
export function TemplateSettings({ accessToken, workspaceId }: { accessToken: string; workspaceId: string }) {
  const confirm = useConfirm();
  const [current, setCurrent] = useTemplate(accessToken, workspaceId);
  const [catalogue] = useTemplateCatalogue(accessToken);
  const [choice, setChoice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const currentId = current?.template.id ?? "general";
  const chosenId = choice ?? currentId;
  const comparing = chosenId !== currentId;
  const [detail] = useTemplateDetail(accessToken, chosenId, comparing);
  const [base] = useTemplateDetail(accessToken, currentId, Boolean(current));
  if (!current) return null;
  const preview = comparing ? detail : current;
  const chosenName = catalogue.find((template) => template.id === chosenId)?.name ?? chosenId;
  const adjusted = current.overrides !== null && typeof current.overrides === "object" && Object.keys(current.overrides as object).length > 0;

  async function save(templateId: string, overrides: Record<string, unknown> | undefined, done: string) {
    setBusy(true);
    try {
      setCurrent(await saveSharedWorkspaceTemplate(accessToken, workspaceId, templateId, overrides));
      setChoice(null);
      showToast("success", done);
    } catch (requestError) {
      showToast("error", apiMessage(requestError, "The workspace template could not be saved."));
    } finally { setBusy(false); }
  }

  async function apply() {
    const agreed = await confirm({
      action: "Change template",
      description: "The sections, names, cards and suggestions change for everyone in this workspace. Nothing stored is moved, created or deleted, and nobody gains or loses access.",
      title: `Use the ${chosenName} template?`,
    });
    if (agreed) await save(chosenId, undefined, `This workspace now uses the ${chosenName} template.`);
  }

  return (
    <section className="shared-settings-group">
      <PanelHeading description={current.source === "fallback" ? undefined : `Version ${current.template.version}`} icon={LayoutList} title="Template" />
      <p className="shared-muted-copy">A template decides which parts of RepoMemo come first in this workspace and what they are called. It never changes who can see or do anything.</p>
      {current.source === "fallback" ? <p className="shared-muted-copy">{current.reason} General is in use until a template is chosen.</p> : null}
      <TemplatePicker disabled={busy} name={`settings-template-${workspaceId}`} onChange={setChoice} templates={catalogue} value={chosenId} />
      {preview ? <dl aria-label={`What the ${chosenName} template arranges`} className="rm-template-preview">
        {previewLines(preview).map((line) => <div key={line.label}><dt>{line.label}</dt><dd>{line.text}</dd></div>)}
      </dl> : null}
      {comparing || current.source !== "workspace" ? <Button busy={busy} disabled={busy || (comparing && !detail)} onClick={() => void apply()} type="button" variant="main">Use {chosenName}</Button> : null}
      {current.source === "workspace" && !comparing && base ? <Adjustments
        adjusted={adjusted}
        base={base}
        busy={busy}
        current={current}
        key={JSON.stringify([current.sections, current.assistant.suggestions])}
        onReset={() => void save(currentId, {}, "The adjustments were removed; the template's own values are back.")}
        onSave={(overrides) => void save(currentId, overrides, Object.keys(overrides).length ? "The adjustments were saved." : "No adjustment differs from the template.")}
      /> : null}
      <ExportTemplate accessToken={accessToken} current={current} key={current.template.id} workspaceId={workspaceId} />
    </section>
  );
}

/** Downloads the workspace's setup as a template file: the template in use with its adjustments, nothing of the workspace's content. */
function ExportTemplate({ accessToken, current, workspaceId }: { accessToken: string; current: WorkspaceTemplateView; workspaceId: string }) {
  const [id, setId] = useState(`${current.template.id}-custom`.slice(0, 40));
  const [name, setName] = useState(`${current.template.name} (custom)`.slice(0, 40));
  const [busy, setBusy] = useState(false);
  async function download() {
    setBusy(true);
    try {
      const { blob, filename } = await downloadSharedWorkspaceTemplateFile(accessToken, workspaceId, { id: id.trim(), name: name.trim() });
      if (await saveBlob(blob, filename)) showToast("success", `${filename} was downloaded. Put it in the folder REPOMEMO_TEMPLATES_DIR names on a server and restart it.`);
    } catch (requestError) {
      showToast("error", apiMessage(requestError, "The template file could not be downloaded."));
    } finally { setBusy(false); }
  }
  return (
    <fieldset className="rm-template-adjust" disabled={busy}>
      <legend>Download as template file</legend>
      <p className="shared-muted-copy">The template this workspace uses, with the adjustments saved on it, as a file another server can load from its templates folder. Nothing from the workspace's content is in it: no files, people or settings.</p>
      <label className="rm-template-adjust-field">Id of the new template
        <Input autoComplete="off" onChange={(event) => setId(event.target.value)} spellCheck={false} value={id} />
        <small>2 to 40 characters: a–z, 0–9 and -. It cannot be an id this server already has.</small>
      </label>
      <label className="rm-template-adjust-field">Name
        <Input autoComplete="off" maxLength={40} onChange={(event) => setName(event.target.value)} value={name} />
      </label>
      <div className="rm-template-adjust-actions">
        <Button busy={busy} disabled={busy || !id.trim() || !name.trim()} onClick={() => void download()} type="button" variant="secondary">Download as template file</Button>
      </div>
    </fieldset>
  );
}

function Adjustments({ adjusted, base, busy, current, onReset, onSave }: {
  adjusted: boolean;
  base: Parameters<typeof adjustmentsFrom>[0];
  busy: boolean;
  current: WorkspaceTemplateView;
  onReset: () => void;
  onSave: (overrides: Record<string, unknown>) => void;
}) {
  const [landing, setLanding] = useState(current.sections.landing);
  const [more, setMore] = useState<string[]>(current.sections.more);
  const [questions, setQuestions] = useState(current.assistant.suggestions.join("\n"));
  const name = (section: WorkspaceSection) => current.labels.sections[section] ?? SECTION_NAMES[section];
  const typed = questionsFromText(questions);
  const tooMany = typed.length > MAX_QUESTIONS;
  const toggle = (section: string, keep: boolean) => setMore((list) => keep ? [...list.filter((entry) => entry !== section), section] : list.filter((entry) => entry !== section));
  return (
    <fieldset className="rm-template-adjust" disabled={busy}>
      <legend>Adjust this workspace</legend>
      <p className="shared-muted-copy">Only what you change here is saved on top of the template; everything else keeps following it.</p>
      <label className="rm-template-adjust-field">Opens on
        <Dropdown aria-label="The section the workspace opens on" onValueChange={setLanding} options={WORKSPACE_SECTIONS.filter((section) => !ADMIN_WORKSPACE_SECTIONS.has(section)).map((section) => ({ label: name(section), value: section }))} value={landing} />
      </label>
      <div aria-label="Sections kept under More" className="rm-template-adjust-more" role="group">
        {WORKSPACE_SECTIONS.map((section) => <Checkbox checked={more.includes(section)} key={section} label={name(section)} onCheckedChange={(keep) => toggle(section, keep)} />)}
      </div>
      <label className="rm-template-adjust-field">Suggested questions, one on each line
        <Textarea aria-invalid={tooMany || undefined} onChange={(event) => setQuestions(event.target.value)} rows={4} value={questions} />
        <small>{tooMany ? `Keep it to ${MAX_QUESTIONS} questions.` : `${typed.length} of ${MAX_QUESTIONS}`}</small>
      </label>
      <div className="rm-template-adjust-actions">
        <Button busy={busy} disabled={busy || tooMany} onClick={() => onSave(adjustmentsFrom(base, { landing, more, suggestions: typed }))} type="button" variant="main">Save adjustments</Button>
        <Button disabled={busy || !adjusted} onClick={onReset} type="button" variant="secondary">Reset to template</Button>
      </div>
    </fieldset>
  );
}
