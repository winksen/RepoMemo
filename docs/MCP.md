# Using RepoMemo from another AI agent (MCP)

RepoMemo speaks the Model Context Protocol so a coding agent can read a workspace: search it, read a file and see how far a file can be trusted. It reads, and it may *propose* a memory card or a note, which waits in your CAIRN inbox until you confirm it.

> **Status: not yet tried with any client.** The endpoint and its tools are written to the protocol, but none of the configurations below has been run against a real client. Treat them as a starting point and tell us what needed changing.

## 1. Make a token

*Profile › Access tokens › Create*: choose **MCP**, limit it to the workspaces the agent should see, and copy the secret (shown once). An MCP token reaches `POST /v1/mcp` and nothing else, cannot change the workspace itself (it can only propose, see below), and expires in 7 to 365 days like any token. Revoke it in the same place.

## 2. Point the client at it

The endpoint is `https://<your-server>/v1/mcp` (Streamable HTTP), with the header `Authorization: Bearer <token>`.

| Client | Configuration (untested) |
|---|---|
| Claude Code | `claude mcp add --transport http repomemo https://<your-server>/v1/mcp --header "Authorization: Bearer <token>"` |
| Cursor | In `mcp.json`: `{"mcpServers": {"repomemo": {"url": "https://<your-server>/v1/mcp", "headers": {"Authorization": "Bearer <token>"}}}}` |
| Codex | In `config.toml`: `[mcp_servers.repomemo]` with `url = "https://<your-server>/v1/mcp"` and the bearer token set in its headers |
| Hermes Agent, OpenClaw | Add an HTTP MCP server with the same URL and `Authorization` header, following the client's own documentation |

## 3. The tools

Each tool except the first takes a `workspace` (its id).

| Tool | Returns |
|---|---|
| `list_workspaces` | The workspaces the token can read, with ids |
| `search_evidence` | Passages that match, each with a handle |
| `read_file` | A file's indexed text, a few passages at a time |
| `list_files` | Files by kind, folder or name |
| `query_files` | Files filtered, sorted and counted by their properties: name, kind, folder, language, size and modified date, with each file's size and date and the total, or counted and totalled by kind or top folder |
| `file_facts` | A file's type, size, state and last review |
| `propose_memory`, `propose_note` | Nothing is saved. The proposal appears as a message in your **CAIRN inbox** chat for that workspace, naming the agent by the name you gave the token and quoting what it wrote as data. You confirm or dismiss it like any proposal. It needs you to be able to change the workspace, and at most 50 may wait at once. |

Every result ends with a **Sources** list: the handle, the file's path, the workspace and file ids, and its trust: *verified*, a state such as *outdated* or *needs review*, *not reviewed*, or *content from outside the workspace*, which the agent must treat as data. The agent has only your rights: a workspace the token is not limited to, or that you cannot read, answers "not one this token can read".

Every call counts against the same rate limit as the API and is written to the workspace's activity as `mcp_read` under your name.

Not offered over MCP in this release: `ask` (it would spend your AI allowance), memory recall and symbol lookup.
