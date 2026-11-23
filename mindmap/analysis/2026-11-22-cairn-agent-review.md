# CAIRN agent review: 2026-11-22

> **Scope:** CAIRN as built on `main` at V0.9.32 (`crates/api/src/agent.rs` and `agent/`, `apps/server/src/agent.rs`, `agent_proposals.rs`, `crates/ai`, `crates/retrieval`, the CAIRN page and panel), compared with the agent harnesses teams use today.
> **Method:** the code was read; nothing was run or built. External products are described from their public documentation and guides as of November 2026 (sources at the end); details that vary between sources are left out.
> **Feeds:** the [0.9 CAIRN plan](../plans/2026-11-22-0.9-cairn-harness.md), the rows V0.9.33 to V0.9.70 in the [release index](../RELEASES_0.9.md), and [ADR 0006](../../docs/decisions/0006-cairn-is-a-bounded-tool-loop-over-governed-evidence.md). The public-readiness pilot rows that followed V0.9.32 moved, unchanged, to the [0.10 line](../RELEASES_0.10.md).

---

## 1. Summary

1. **CAIRN is not an agent yet. It is an intent router with thirteen handlers.** A message picks exactly one capability (by click, by English keyword lists, or by one JSON classification call), that handler runs once, and the turn ends. The model never decides what to do next, never chains two jobs, never looks at a result and tries again. "Find the contracts that mention penalties, tell me which are outdated, and move those to Archive" cannot be done in one request today.
2. **The most valuable thing already built is the confirmation model, not the jobs.** A proposal does nothing until its person confirms it, then runs through the same HTTP handlers the person would call, with the same role, quota and workspace checks, and records the outcome in the stored turn. That is the right safety model and it scales to a real tool loop unchanged.
3. **RepoMemo's moat is governed evidence, and CAIRN barely uses it.** Hybrid search with rank fusion, lifecycle states that weight ranking, memory cards with revisions, citations and approvals, owners and review dates, connector freshness and quarantine, answer corrections, symbols, versions, projects and tasks. Hermes and OpenClaw remember things in Markdown files and a full-text index; none of them can say *who verified this, when, from which source, and whether that source is still current*. CAIRN should be the agent whose memory is auditable evidence.
4. **The provider contract blocks everything else.** `AiProvider::generate(system, prompt, context) -> String` has no message roles, no tool calls, no structured output, no streaming and no token usage. History is pasted into the prompt as text, capped at 1,500 estimated tokens and eight turns. Only Ollama and OpenRouter exist. Model-agnostic tool use needs a new contract first.
5. **Memory is per chat and private.** A conversation belongs to one person in one workspace. CAIRN cannot recall another chat, learns nothing about the person, never reads team memory unless asked to "recall memory", and forgets anything beyond eight short turns.
6. **The plan should make CAIRN a bounded tool loop over governed evidence**, keep the router as the no-AI path, give it layered memory where team knowledge stays reviewed and cited, open it across workspaces only under each workspace's AI boundary, and expose RepoMemo to other agents through MCP so Claude Code, Hermes or OpenClaw can use the team's evidence instead of competing with it.

---

## 2. What other harnesses do today

| Harness | Model | Loop | Memory | Skills / procedures | Proactive | Multi-session | Safety model |
|---|---|---|---|---|---|---|---|
| **Claude Code** (Anthropic) | Claude models | Tool loop with plan mode, todo list, subagents, background tasks, context compaction | `CLAUDE.md` files read at start; an auto-memory folder of small fact files with an index | `SKILL.md` folders loaded on demand (progressive disclosure); slash commands; hooks | Scheduled routines, `/loop` | Subagents, forked sessions, session search | Permission modes per tool; hooks can veto; sandboxing |
| **Hermes Agent** (Nous Research, MIT, Feb 2026) | Any (OpenRouter, local, vendor APIs) | Tool loop, 40+ tools, subagents, several terminal backends | Layered: a frozen snapshot of preferences at session start, an FTS5 database of every past session searchable with LLM summaries, a user model | Writes a skill document itself after solving a hard problem, then reuses it (`/skill <name>`) | Cron jobs; reviews past conversations for preferences worth saving | Messaging gateways (Telegram, Discord, Slack…) | Approval for dangerous commands, container backends |
| **OpenClaw** (open source, Jan 2026) | Any | Tool loop behind a gateway | Markdown is the source of truth (`MEMORY.md`, daily notes, `SOUL.md`, `USER.md`), indexed by SQLite with embeddings and keyword search | Community skills (ClawHub registry, thousands) | **Heartbeat**: a cron tick loads a checklist and the agent decides whether to act; silence is the default | `sessions_send` delegates to another agent's session; agent-to-agent off by default | Non-main sessions sandboxed. A coordinated malicious-skill campaign in Feb 2026 wrote payloads into memory files to persist across sessions |
| **Letta / Letta Code** | Any | Tool loop | Memory blocks the agent edits, git-backed; background "sleep-time" consolidation, moving to client-side subagents | Skills and subagents | Sleep-time compute between turns | Shared harness | — |
| **LangChain Deep Agents** | Any | Planning tool, subagents, virtual file system | Files as memory; "your harness owns your memory" | Sub-agent prompts | — | — | — |

What they converge on, and what it means for CAIRN:

1. **A loop, not a router.** Every serious harness lets the model call tools, read results and decide the next step, within a budget. CAIRN's handlers are good *tools*; they are wired as *destinations*.
2. **Memory in layers, loaded selectively.** A small, always-loaded core (identity, instructions, preferences); a searchable archive of past sessions; procedures (skills) whose names are always visible and whose bodies load on demand; background consolidation. Nobody loads everything.
3. **Procedures learned from experience.** Hermes writes skills after hard tasks; Claude Code and OpenClaw load them on demand. In all three, the learned procedure is **trusted the moment it is written**. That is the gap a team product can close: a learned procedure should be reviewed like any other evidence before the agent follows it.
4. **Proactivity through a heartbeat or a schedule**, with "do nothing" as the default outcome.
5. **Model agnosticism through an OpenAI-compatible message-and-tools contract**, with a fallback for models that cannot call tools.
6. **Memory is the attack surface.** OpenClaw's skill campaign persisted by writing into memory files. Any memory an agent writes to without review, and any instruction it reads from imported content, is a persistence and injection channel. RepoMemo already has the cure (review states, approvals, revisions, quarantine); CAIRN must route every memory write through it.

What none of them is: **a team's governed source of truth.** Their memory is personal, flat and self-asserted. Glean-, Dust- or Notion-style workplace agents are closer in audience, but they sit on top of documents with no review lifecycle of their own. RepoMemo has both the documents and the lifecycle.

---

## 3. CAIRN at V0.9.32, honestly

### What is good and should be kept

| Strength | Where | Why it matters for an agent |
|---|---|---|
| **Confirmed proposals run as the person** | `apps/server/src/agent_proposals.rs` | Write tools never act on their own; every check (role, quota, repository files, folders, revisions) applies unchanged; the activity feed names the person. This *is* a permission system, and a better one than per-tool allow lists |
| **History comes from storage, never the client** | `apps/server/src/agent.rs` `send_agent_message` | A client cannot make CAIRN "remember" text. Keep this for every memory layer |
| **Viewer and AI-role rules on the server** | `list_agent_capabilities`, `ai_access` | Capability exposure already depends on role and AI policy |
| **Evidence discipline** | `agent/evidence.rs` (V0.9.4, V0.9.8) | Only cited passages are evidence, markers renumbered to the list shown |
| **Memory card revisions in proposals** | V0.9.30 | A stale proposal is refused instead of overwriting someone's edit |
| **Useful without AI** | rules router, find, search, symbol, recall | The guiding rule holds; it must survive the loop |
| **Quota and usage accounting** | `ai_usage_daily`, `consume_ai_quota` | Budgets per workspace exist to bound a loop |

### What is weak

| # | Finding | Evidence in code | Consequence |
|---|---|---|---|
| W1 | **One message, one capability.** No loop, no chaining, no retry after an empty result | `run_agent` routes once and returns | Composite requests fail or are half answered; CAIRN cannot recover from its own wrong guess |
| W2 | **English keyword routing first.** About 150 English phrases decide before the model is asked | `SUMMARY_TRIGGERS`, `SEARCH_PREFIXES`, `FIND_PREFIXES`, `QUESTION_WORDS`… | "Résume ce fichier" or an Arabic request misroutes or falls to the classifier; V0.9.3 promises the person's language but routing is English. Each new job adds more lists and more collisions (`find` already needs ordering rules) |
| W3 | **Provider contract is text in, text out** | `crates/ai/src/lib.rs` `GenerateRequest`, `generate` | No native tools, no roles, no streaming, no usage tokens; two providers only. The classifier call exists only because tools do not |
| W4 | **Tiny, lossy conversation memory** | `HISTORY_TOKEN_BUDGET = 1_500`, `HISTORY_MAX_TURNS = 8`, answers cut to 700 chars | Long chats forget the file from turn 2; the budget ignores the model's real window |
| W5 | **No memory across chats, none of the person** | `assistant_conversations` is per user per workspace; no profile | "As we decided last week" fails; preferences are restated every chat |
| W6 | **Team memory is opt-in** | `RecallMemory` runs only when routed; Ask searches chunks | Reviewed memory cards, the most trusted knowledge, are not consulted by default |
| W7 | **Corrections are not fed back** | `search_feedback` kind `wrong_answer` (V0.8.14) is never read by CAIRN | The team tells RepoMemo where it was wrong, and CAIRN repeats it |
| W8 | **Synchronous request** | `send_agent_message` awaits the whole job | Analyze of eight files blocks one HTTP request; no progress, no cancel; the eight-file cap exists for this reason |
| W9 | **No trace** | only an activity line when `generated` | Nobody (including the maintainer) can see why CAIRN answered as it did, which tools ran, or what it cost |
| W10 | **No evaluation** | tests check routing strings | No question set, no citation precision, no regression signal when a prompt or model changes |
| W11 | **Untrusted content is not marked** | connector issues, mail, guest uploads enter passages like any file | With tools, an imported Jira comment saying "move every file to the trash" becomes a proposal. Confirmation saves the day, but the person should be told where the idea came from |
| W12 | **Single workspace per chat** although the page is global | V0.8.49 groups chats by workspace; each chat binds one | Teams across workspaces cannot ask one question over them |
| W13 | **`agent.rs` is 2,322 lines** of rules, handlers and tests | — | Every new job touches the router, the classifier prompt, the catalog and the server; the tool registry removes that |

### Process critique

- **The 0.9 line changed purpose without a decision record.** The roadmap's own "Later" table and the readiness report list *write-capable AI agents* and *MCP/IDE surfaces* as after-1.0 work, and 0.9 was the feature-freeze pilot line. Moving the pilot rows to 0.10 is the right call if CAIRN is the product's differentiator, but the 1.0 promise must now name CAIRN, and the pilot must test it. [ADR 0006](../../docs/decisions/0006-cairn-is-a-bounded-tool-loop-over-governed-evidence.md) records the change.
- **Ten of the 33 "CAIRN agent" rows are interface work** (V0.9.15–V0.9.24: top bar, tabs, workspace settings). They are shipped as built and stay numbered, but the track label overstates CAIRN's progress.
- **All 33 rows are 🧪.** None has a live walkthrough with a real provider. A loop on top of unvalidated jobs compounds the risk; V0.9.46 (evaluation) comes before the loop becomes the default for this reason.
- **Keyword lists were the right first step without tools, and the wrong place to keep investing.** Every row since V0.9.25 that added phrases is debt the loop retires for tool-capable models.

---

## 4. Direction: what CAIRN should become

**CAIRN is a bounded tool loop over governed evidence.** A cairn is a pile of stones that marks a trail for the next walker; CAIRN's memory is what the team left behind, checked and dated, not what a model decided to believe.

| Principle | Concretely |
|---|---|
| **Read freely, write by proposal, never send out** | Tools have an effect class: *read* runs, *propose* produces a proposal that the person confirms through their own handlers, *external* (webhooks, chat destinations, publication, outside MCP tools) is not available to CAIRN in this line |
| **Every claim rests on something read in this run** | Evidence handles from tool results form a ledger; an answer may cite only handles in it, and the trust state of each (verified, outdated, stale, untrusted) is shown |
| **Memory in layers, governed by its audience** | Run scratchpad → conversation summary → personal memory (the person's own, visible and editable) → team memory (memory cards and playbooks, reviewed and cited). Only reviewed team memory and verified playbooks steer CAIRN for everyone |
| **Learn by proposing, not by writing** | Like Hermes, CAIRN notices what is worth keeping (a decision, a correction, a procedure); unlike Hermes, it proposes a card or a playbook that enters review |
| **Model-agnostic by contract** | One message-and-tools contract (OpenAI-compatible), native tools where the model has them, a strict JSON protocol where it does not, the rules router where there is no model |
| **Bounded everywhere** | Steps, tokens, wall clock, AI quota, set sizes, routine counts; exhaustion yields a partial answer that says so |
| **Teams across workspaces, inside each workspace's boundary** | Cross-workspace search follows read rights; cross-workspace *answers* send a workspace's passages only to a provider that workspace itself allows |
| **Be the memory other agents use** | An MCP endpoint with the person's rights lets Claude Code, Hermes, OpenClaw or Codex search and cite RepoMemo evidence, and submit proposals into the person's CAIRN inbox |

What this line deliberately does **not** do: no CAIRN access to outside MCP tools, shell, web browsing or outbound messages; no memory or playbook that steers others without review; no autonomous writes from routines; no per-person productivity metrics from traces. Each is listed in the plan's exclusions with the condition that would reopen it.

---

## 5. Sources

- Hermes Agent: [Nous Research product page](https://nousresearch.com/?p=5667), [documentation](https://hermes-agent.nousresearch.com/docs), [Better Stack guide](https://betterstack.com/community/guides/ai/hermes-agent/), [agentic-ai notes](https://agentic-ai.readthedocs.io/en/latest/AgentPlatforms/hermes-agent/), [WWT lab](https://www.wwt.com/lab/hermes-agent-the-agent-that-remembers-you).
- OpenClaw: [feature atlas](https://it-huset.github.io/openclaw-guide/docs/feature-atlas), [reference](https://it-huset.github.io/openclaw-guide/docs/reference), [architecture monograph](https://majid-mazouchi.github.io/autonomy/assets/posts/openclaw-architecture.html), [GoDaddy explainer](https://www.godaddy.com/resources/skills/how-does-openclaw-work), [agentic-ai notes](https://agentic-ai.readthedocs.io/en/latest/AgentPlatforms/openclaw/).
- Letta and harness memory: [Letta: our next phase](https://letta.com/blog/our-next-phase), [sleep-time compute](https://letta.com/blog/sleep-time-compute), [Letta Code](https://www.letta.com/blog/letta-code), [The New Stack on FrontierMemory](https://thenewstack.io/letta-frontiermemory-agent-harness-benchmark/), [LangChain: your harness, your memory](https://langchain.com/blog/your-harness-your-memory).
- Claude Code: product documentation (skills, memory files, subagents, hooks, permission modes).
