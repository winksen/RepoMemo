# 0006: CAIRN Is a Bounded Tool Loop Over Governed Evidence

Status: proposed; amended by [ADR 0008](0008-cairn-asks-the-model-first-whenever-there-is-one.md) (the router is no longer the default while a model is available)

Date: 2026-11-22

## Context

Up to V0.9.32, CAIRN routes each message to one of thirteen jobs, by a click, by English keyword lists or by one classification call, and runs that job once. Jobs that would change the workspace return a proposal that the person confirms; the confirmation runs through the person's own handlers.

Teams now expect an assistant to chain steps, recover from an empty result, remember across conversations and work across workspaces. Agent harnesses that do this (Claude Code, Hermes Agent, OpenClaw, Letta) let the model call tools in a loop and keep layered memory, but their memory and learned procedures are trusted as soon as they are written, which has already been used to persist malicious instructions.

The roadmap had placed write-capable agents and MCP surfaces after 1.0, and the 0.9 line was the pilot line. The pilot rows move to 0.10 so that 0.9 can finish CAIRN; the pilot then tests CAIRN as part of the 1.0 promise.

## Decision

1. **Loop.** With a provider that can call tools (natively or through a strict JSON protocol), CAIRN runs a loop: the model calls tools, reads their results and decides the next step, within limits on steps, tokens, time and AI quota. Without a provider, the existing rules router stays and remains useful.
2. **Effect classes.** Every tool is *read* (runs), *propose* (returns a proposal; nothing changes until the person confirms, and confirmation runs the person's own handler) or *external*. No external tool (outbound webhook, chat destination, publication, outside MCP tool, shell, web) is given to CAIRN in the 0.9 line.
3. **Evidence ledger.** Tool results carry evidence handles. An answer may cite only handles returned in its run; unknown handles are removed and reported. Each cited source shows its trust state.
4. **Governed memory.** Memory is layered: run scratchpad, conversation summary, personal memory (one person's, visible and editable by them), team memory (memory cards and playbooks). Only team memory that people reviewed steers CAIRN for others. CAIRN proposes cards, corrections and playbooks; it never writes team memory directly.
5. **Untrusted content is data.** Passages from connectors, mail and guest uploads are marked untrusted, are never read as instructions, and a proposal made after reading them says so.
6. **Workspace boundary.** A chat may span the workspaces a person can read. A workspace's passages are sent only to a provider that the workspace itself has enabled for that person's role; otherwise they are shown as search results, not sent.
7. **Model agnostic.** The provider contract is chat messages with optional tools, OpenAI-compatible; Ollama, OpenRouter and any OpenAI-compatible endpoint implement it.
8. **RepoMemo serves other agents.** An MCP endpoint exposes read tools with the token owner's rights; outside agents can submit proposals into the owner's CAIRN inbox, never apply them.

## Consequences

- The keyword router stops growing. Its lists stay for the no-AI path and for greetings, and are no longer consulted before the model when the loop is on.
- Each run is stored with its steps, so behaviour can be explained, evaluated and costed. Traces are personal like conversations, follow conversation deletion and account anonymization, and are not part of workspace archives.
- New tables (runs and steps, personal memory, chat search, chat shares, routines, inbox) each carry their own migration, retention and anonymization handling.
- A loop costs more AI calls than a router. Budgets are per run and per workspace, and the default stays the router until the evaluation set passes for the configured model.
- The 1.0 promise names CAIRN. The 0.10 pilot observes it; a CAIRN feature without passing evaluation and pilot evidence is narrowed or deferred rather than claimed.
