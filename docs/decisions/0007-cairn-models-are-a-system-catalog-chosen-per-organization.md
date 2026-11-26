# 0007: CAIRN's Models Are a System Catalog, Chosen per Organization and Narrowed per Workspace

Status: proposed

Date: 2026-11-26

## Context

Up to V0.9.69, each workspace administrator adds AI providers in the workspace's Settings › AI, and CAIRN, Ask and summaries use the workspace's first enabled text provider with one fixed model. Agent mode is a flag on that provider. CAIRN's budgets and retention are environment variables, and nothing about CAIRN can be set for an organization as a whole.

An organization of many workspaces therefore repeats the same configuration and the same API key in each, people cannot choose between models, and a rule for CAIRN (a limit, a turned-off job, an instruction) has no single place to live. The [administration plan](../../mindmap/plans/2026-11-26-0.9-cairn-administration.md) gives CAIRN an administration page and needs a rule for where each setting belongs.

## Decision

1. **One catalog per server.** Connections (endpoint, key, locality) and the models they offer are administered by system administrators and by people they make CAIRN administrators. Keys exist once, encrypted, and are never read back.
2. **Organizations choose, workspaces narrow, people pick.** An organization enables catalog models and sets its default and Agent mode; its workspaces inherit them. A workspace administrator can offer fewer of them and set the workspace default, never add others. A person picks a model per chat from what is offered to their role.
3. **Limits only tighten downward.** System caps bound organizations, which bound workspaces. A setting left empty inherits. Preferences are defaults that a person may override; limits are not.
4. **One text model resolution.** CAIRN, Ask and summaries use the same resolved text model. Embedding and vision models stay a workspace choice from the catalog, because changing them rebuilds stored data.
5. **No silent move to the cloud.** A local model never falls back to a cloud one, a cloud connection needs the system's acknowledgement, and a refused or vanished model choice is said in the chat.
6. **Abilities are data over built-in tools.** A custom ability is instructions, a subset of the registered tools and an output form. It cannot add a tool, an external effect or a right the person lacks (ADR 0006, decision 2).
7. **Administrators see settings and aggregates, not conversations.** Usage is counted per organization, workspace and model; chats, traces and personal memory stay personal.

## Consequences

- Existing workspace providers are migrated into the catalog and each workspace keeps its former model as offered and default, so an upgrade changes nothing a person sees until an administrator changes it.
- The workspace provider routes become read-only, and the API guide records the change.
- Workspace CAIRN settings travel in workspace archives; catalog and organization settings do not, and an imported workspace drops model ids its new server does not know, with a warning.
- Every refusal names the tier that set it, so people know whom to ask.
- A server with one organization and one workspace still works with no administration at all: the migrated provider is the organization's default.
