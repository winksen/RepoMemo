# HTTP API reference

> **Branch of:** [TECHNICAL_MINDMAP.md §5](../TECHNICAL_MINDMAP.md#5-http-api-reference-167-operations-under-v1) · **Generated** from the router and the handler signatures by `node scripts/list-routes.mjs --markdown`; the guard column is read from the handler bodies, so check a route's handler before relying on a row · **Status:** V0.2.6 (the two index routes edited by hand; regenerate before the next release candidate)

162 routes of the main router, plus the platform routes (`/v1/platform/*`, [environment.rs](../../apps/server/src/environment.rs), served before an environment is attached) and the avatar routes (`/v1/users/{id}/avatar`, `/v1/profile/avatar`, [avatars.rs](../../apps/server/src/avatars.rs)).

## Conventions

| | |
|---|---|
| **Base** | JSON over HTTPS, `Content-Type: application/json`. File uploads send the bytes as the body with `X-RepoMemo-Filename` (percent-encoded) and optionally `X-RepoMemo-Folder-Id` |
| **Authentication** | `Authorization: Bearer <access token>` (a JWT from sign-in) or a personal access token (`rmp_…`). The web app also sends `X-RepoMemo-Client: web`, which makes the refresh token travel in an `HttpOnly` cookie |
| **Authentication column** | *public*: none. *auth*: any session or token (read-only tokens only on GET and HEAD). *reading*: like auth, and a read-only token may make this POST because it only reads. *session*: a signed-in session only, never a token. *signed link*: the link's own token |
| **Guards** | `require_workspace_read/write/admin/owner`, `require_organization_read/admin/owner`: the caller's role in that workspace or organization; `require_system_admin`: system or app administrators; `require_full_system_admin`: system administrators only; `require_system_change`: a session or a `system` token |
| **Errors** | `{"error": {"code": "bad_request \| unauthorized \| invalid_credentials \| forbidden \| not_found \| conflict \| rate_limited \| internal_error", "message": "…"}}`. Quota and limit codes seen since V0.1.74: `workspace_full` (413), `workspace_limit` (409), `disk_low` (507), `rate_limited` (429, with `Retry-After`) |
| **Paging** | `?limit=&offset=` (at most 500) on `artifacts/query` and the artifact list; the total is in `X-Total-Count` |
| **Limits** | Every route but the event streams answers within `REPOMEMO_REQUEST_TIMEOUT_SECONDS` (408). Searches, figures, the map, health and the file query are rate limited per person (`REPOMEMO_HEAVY_REQUESTS_PER_MINUTE`) |
| **Live events** | `GET /v1/workspaces/{id}/events` and `GET /v1/system/events`: server-sent events (`job`, `activity`, `resync`); a person may hold `REPOMEMO_MAX_EVENT_STREAMS_PER_USER` open |

## Health and setup

| Method | Path | Authentication | Guards | Request body | Response |
|---|---|---|---|---|---|
| GET | `/health` | public | — | — | HealthResponse |
| GET | `/health/ready` | public | — | — | ReadinessResponse |
| GET | `/v1/setup` | public | — | — | SetupStatus |
| POST | `/v1/setup/verify-code` | public | — | VerifyCodeRequest | status |
| POST | `/v1/setup/admin` | public | — | CreateAdminRequest | TokenResponse |
| GET | `/v1/setup/checks` | auth | finishing administrator | — | Vec<SetupCheck |
| POST | `/v1/setup/complete` | auth | finishing administrator | — | SetupStatus |

## Sign-in, sessions and access links

| Method | Path | Authentication | Guards | Request body | Response |
|---|---|---|---|---|---|
| POST | `/v1/auth/register` | public | — | RegisterRequest | TokenResponse |
| POST | `/v1/auth/login` | public | — | LoginRequest | TokenResponse |
| POST | `/v1/auth/refresh` | public | — | RefreshRequest | TokenResponse |
| POST | `/v1/auth/logout` | public | — | RefreshRequest | status |
| POST | `/v1/auth/logout-all` | session | — | — | status |
| POST | `/v1/auth/forgot-password` | public | — | ForgotPasswordRequest | 202 ForgotPasswordAnswer (the same for every address) |
| POST | `/v1/auth/password-reset/preview` | public | — | TokenBody | PasswordResetPreview |
| POST | `/v1/auth/password-reset` | public | — | CompletePasswordReset | TokenResponse |
| POST | `/v1/invitations/preview` | public | — | TokenBody | InvitationPreview |
| POST | `/v1/invitations/accept` | session | — | TokenBody | AcceptedInvitation |
| DELETE | `/v1/invitations/{invitation_id}` | auth | require_workspace_admin, require_organization_admin | — | status |
| GET | `/v1/profile/tokens` | session | — | — | Vec<PersonalAccessToken |
| POST | `/v1/profile/tokens` | session | — | CreateTokenRequest | CreatedToken |
| DELETE | `/v1/profile/tokens/{token_id}` | session | — | — | status |
| GET | `/v1/session` | auth | — | — | SharedSession |

## Profile and notifications

| Method | Path | Authentication | Guards | Request body | Response |
|---|---|---|---|---|---|
| GET | `/v1/profile` | auth | — | — | UserProfileResponse |
| PUT | `/v1/profile` | auth | — | UpdateProfileRequest | SharedUser |
| POST | `/v1/profile/password` | session | — | ChangePasswordRequest | TokenResponse |
| GET | `/v1/profile/tasks` | auth | — | — | Vec<CollaborationTask |
| GET | `/v1/notifications` | auth | — | — | Vec<SharedNotification |
| POST | `/v1/notifications/read-all` | auth | — | — | status |
| POST | `/v1/notifications/{notification_id}/read` | auth | — | — | SharedNotification |

## System administration

| Method | Path | Authentication | Guards | Request body | Response |
|---|---|---|---|---|---|
| GET | `/v1/system/overview` | auth | require_system_admin | — | — |
| GET | `/v1/system/usage` | auth | require_system_admin | — | UsageResponse |
| GET | `/v1/system/users` | auth | require_system_admin | — | Vec<SystemUser |
| PUT | `/v1/system/users/{user_id}/system-admin` | session | require_full_system_admin | SystemAdminRequest | status |
| PUT | `/v1/system/users/{user_id}/app-admin` | session | require_full_system_admin | SystemAdminRequest | status |
| POST | `/v1/system/users/{user_id}/sessions/end` | auth | require_full_system_admin, require_system_change | — | status |
| POST | `/v1/system/users/{user_id}/unlock` | auth | require_full_system_admin, require_system_change | — | Value |
| POST | `/v1/system/users/{user_id}/password-reset` | session | require_full_system_admin | — | IssuedPasswordReset |
| GET | `/v1/system/settings` | auth | require_system_admin | — | SettingsResponse |
| PUT | `/v1/system/settings` | session | require_full_system_admin | UpdateSettingsRequest | SettingsResponse |
| DELETE | `/v1/system/settings/{key}` | session | require_full_system_admin | — | SettingsResponse |
| POST | `/v1/system/environment/detach` | session | require_full_system_admin | — | status |
| GET | `/v1/system/logs` | auth | require_system_admin | — | LogsResponse |
| GET | `/v1/system/logs/files` | auth | require_system_admin | — | LogFilesResponse |
| GET | `/v1/system/logs/files/{day}` | auth | require_full_system_admin | — | — |
| GET | `/v1/system/audit` | auth | require_system_admin | — | Vec<SystemAuditEvent |
| GET | `/v1/system/jobs` | auth | require_system_admin | — | Vec<SystemJob |
| GET | `/v1/system/maintenance` | auth | require_system_admin | — | MaintenanceStatus |
| POST | `/v1/system/maintenance/run` | auth | require_system_admin, require_system_change | — | MaintenanceStatus |
| GET | `/v1/system/backups` | auth | require_system_admin | — | BackupsResponse |
| POST | `/v1/system/backups` | auth | require_full_system_admin, require_system_change | — | BackupsResponse |
| DELETE | `/v1/system/backups/{name}` | auth | require_full_system_admin, require_system_change | — | status |
| POST | `/v1/system/backups/{name}/restore` | auth | require_full_system_admin, require_system_change | RestoreRequest | BackupsResponse |
| GET | `/v1/system/metrics` | auth | require_full_system_admin | — | — |
| GET | `/v1/system/console` | auth | require_system_admin | — | WelcomeResponse |
| POST | `/v1/system/console` | reading | require_system_admin (+ require_system_change for --yes) | — | — |
| GET | `/v1/system/events` | auth | require_system_admin | — | — |

## Organizations

| Method | Path | Authentication | Guards | Request body | Response |
|---|---|---|---|---|---|
| GET | `/v1/organizations` | auth | — | — | Vec<Organization |
| POST | `/v1/organizations` | auth | — | CreateOrganizationRequest | Organization |
| PUT | `/v1/organizations/{organization_id}` | auth | require_organization_owner | UpdateOrganizationRequest | Organization |
| GET | `/v1/organizations/{organization_id}/members` | auth | require_organization_read | — | Vec<OrganizationMember |
| PUT | `/v1/organizations/{organization_id}/members` | auth | require_organization_admin | UpsertOrganizationMemberRequest | OrganizationMember |
| GET | `/v1/organizations/{organization_id}/invitations` | auth | require_organization_admin | — | Vec<Invitation |
| POST | `/v1/organizations/{organization_id}/invitations` | auth | require_organization_admin | CreateInvitationRequest | CreatedInvitation |
| DELETE | `/v1/organizations/{organization_id}/members/{user_id}` | auth | require_organization_admin | — | status |
| POST | `/v1/organizations/{organization_id}/transfer-ownership` | session | require_organization_owner | TransferOwnershipRequest | status |

## Dashboard and workspaces

| Method | Path | Authentication | Guards | Request body | Response |
|---|---|---|---|---|---|
| GET | `/v1/dashboard` | auth | — | — | DashboardResponse |
| GET | `/v1/workspaces` | auth | — | — | Vec<SharedWorkspace |
| POST | `/v1/workspaces` | auth | — | CreateWorkspaceRequest | SharedWorkspace |
| PUT | `/v1/workspaces/{workspace_id}` | auth | require_workspace_owner | UpdateWorkspaceRequest | Workspace |
| DELETE | `/v1/workspaces/{workspace_id}` | auth | require_workspace_owner | — | status |

## Evidence, folders and files

| Method | Path | Authentication | Guards | Request body | Response |
|---|---|---|---|---|---|
| GET | `/v1/workspaces/{workspace_id}/artifacts` | auth | require_workspace_read | — | Vec<SharedArtifactSummary |
| GET | `/v1/workspaces/{workspace_id}/folders` | auth | require_workspace_read | — | Vec<Folder |
| POST | `/v1/workspaces/{workspace_id}/folders` | auth | require_workspace_write | CreateFolderRequest | Folder |
| PUT | `/v1/artifacts/{artifact_id}/folder` | auth | require_workspace_write | MoveArtifactRequest | SharedArtifactSummary |
| GET | `/v1/artifacts/{artifact_id}/file` | auth | require_workspace_read | — | — |
| GET | `/v1/artifacts/{artifact_id}/document-preview` | auth | require_workspace_read | — | DocumentPreview |
| GET | `/v1/artifacts/{artifact_id}/rendered-preview/status` | auth | require_workspace_read | — | RenderStatusResponse |
| GET | `/v1/artifacts/{artifact_id}/rendered-preview` | auth | require_workspace_read | — | — |
| POST | `/v1/artifacts/{artifact_id}/open-link` | auth | require_workspace_read | — | FileLinkResponse |
| GET | `/v1/shared-files/{token}/{filename}` | signed link | link token, first client only | — | — |
| GET | `/v1/workspaces/{workspace_id}/artifacts/index-failures` | auth | require_workspace_read | — | Vec<ArtifactIndexFailure |
| POST | `/v1/workspaces/{workspace_id}/artifacts/query` | reading | require_workspace_read | QueryArtifactsRequest | Vec<ArtifactSummary |
| POST | `/v1/workspaces/{workspace_id}/artifacts/text` | auth | require_workspace_write | CreateTextArtifactRequest | SharedArtifactSummary |
| POST | `/v1/workspaces/{workspace_id}/artifacts/upload` | auth | require_workspace_write | — | SharedArtifactSummary |
| GET | `/v1/artifacts/{artifact_id}` | auth | require_workspace_read | — | ArtifactDetail |
| PUT | `/v1/artifacts/{artifact_id}` | auth | require_workspace_write | UpdateArtifactRequest | ArtifactSummary |
| DELETE | `/v1/artifacts/{artifact_id}` | auth | require_workspace_write | — | status |
| GET | `/v1/artifacts/{artifact_id}/chunks` | auth | require_workspace_admin | — | Vec<Chunk |
| GET | `/v1/artifacts/{artifact_id}/comments` | auth | require_workspace_read | — | Vec<ArtifactComment |
| POST | `/v1/artifacts/{artifact_id}/comments` | auth | require_workspace_write | SaveArtifactCommentRequest | ArtifactComment |
| GET | `/v1/artifacts/{artifact_id}/lifecycle` | auth | require_workspace_read | — | ArtifactLifecycle |
| PUT | `/v1/artifacts/{artifact_id}/lifecycle` | auth | require_workspace_write | SaveArtifactLifecycleRequest | ArtifactLifecycle |
| GET | `/v1/artifacts/{artifact_id}/lifecycle/history` | auth | require_workspace_read | — | Vec<ArtifactLifecycleEvent |
| POST | `/v1/artifacts/{artifact_id}/index` | auth | require_workspace_write | — | 202 IndexingJobStatus (running; done in the background) |
| POST | `/v1/workspaces/{workspace_id}/index` | auth | require_workspace_admin | — | 202 IndexingJobStatus (running; 409 while one runs) |
| POST | `/v1/workspaces/{workspace_id}/artifacts/bulk` | auth | require_workspace_write | BulkArtifactsRequest | BulkArtifactsResponse |
| POST | `/v1/workspaces/{workspace_id}/folders/{folder_id}/move` | auth | require_workspace_write | MoveFolderRequest | Folder |
| POST | `/v1/artifacts/{artifact_id}/versions` | auth | require_workspace_write | — | SharedArtifactSummary |

## Retrieval, memory and assistant

| Method | Path | Authentication | Guards | Request body | Response |
|---|---|---|---|---|---|
| POST | `/v1/workspaces/{workspace_id}/ai-overview` | auth | require_workspace_read, authorize_ai_use | — | WorkspaceAiOverview |
| POST | `/v1/workspaces/{workspace_id}/ask` | reading | require_workspace_read, authorize_ai_use | AskWorkspaceRequest | AskAnswer |
| GET | `/v1/workspaces/{workspace_id}/knowledge-map` | auth | require_workspace_read | — | KnowledgeMap |
| GET | `/v1/workspaces/{workspace_id}/health` | auth | require_workspace_read | — | WorkspaceHealth |
| POST | `/v1/workspaces/{workspace_id}/health/actions` | auth | require_workspace_admin | HealthActionRequest | HealthActionResult |
| GET | `/v1/workspaces/{workspace_id}/agent/capabilities` | auth | require_workspace_read | — | AgentCapabilitiesResponse |
| POST | `/v1/workspaces/{workspace_id}/agent/messages` | auth | require_workspace_read, consume_ai_quota | AgentMessageRequest | AgentTurnResponse |
| GET | `/v1/workspaces/{workspace_id}/agent/conversations` | auth | require_workspace_read | — | Vec<AgentConversation |
| GET | `/v1/agent/conversations/{conversation_id}` | auth | require_workspace_read | — | AgentConversationDetail |
| PUT | `/v1/agent/conversations/{conversation_id}` | auth | require_workspace_read | RenameConversationRequest | AgentConversation |
| DELETE | `/v1/agent/conversations/{conversation_id}` | auth | require_workspace_read | — | status |
| GET | `/v1/workspaces/{workspace_id}/retrieval-facets` | auth | require_workspace_read | — | RetrievalFacetsResponse |
| POST | `/v1/workspaces/{workspace_id}/search` | reading | require_workspace_read | SearchWorkspaceRequest | Vec<SearchResult |
| GET | `/v1/workspaces/{workspace_id}/symbols` | auth | require_workspace_read | — | Vec<repomemo_domain::SymbolSearchResult |
| GET | `/v1/workspaces/{workspace_id}/memory-cards` | auth | require_workspace_read | — | Vec<MemoryCardSummary |
| POST | `/v1/workspaces/{workspace_id}/memory-cards` | auth | require_workspace_write | CreateMemoryCardBody | MemoryCard |
| POST | `/v1/workspaces/{workspace_id}/memory-cards/search` | reading | require_workspace_read | SearchMemoryCardsRequest | Vec<MemoryCardSummary |
| GET | `/v1/memory-cards/{card_id}` | auth | require_workspace_read | — | MemoryCardDetail |
| PUT | `/v1/memory-cards/{card_id}` | auth | require_workspace_write | UpdateMemoryCardBody | MemoryCard |
| DELETE | `/v1/memory-cards/{card_id}` | auth | require_workspace_write | — | status |
| GET | `/v1/memory-cards/{card_id}/export` | auth | require_workspace_read | — | — |

## Repositories

| Method | Path | Authentication | Guards | Request body | Response |
|---|---|---|---|---|---|
| GET | `/v1/workspaces/{workspace_id}/repositories` | auth | require_workspace_read | — | — |
| POST | `/v1/workspaces/{workspace_id}/repositories` | auth | require_workspace_admin | RepositoryLinkRequest | RepositorySyncResponse |
| POST | `/v1/workspaces/{workspace_id}/repositories/check` | auth | require_workspace_admin | RepositoryLinkRequest | RepoAccessCheck |
| GET | `/v1/repositories/{source_id}` | auth | require_workspace_read | — | RepoSource |
| PUT | `/v1/repositories/{source_id}` | auth | require_workspace_admin | UpdateRepositoryRequest | RepoSource |
| DELETE | `/v1/repositories/{source_id}` | auth | require_workspace_admin | — | status |
| POST | `/v1/repositories/{source_id}/sync` | auth | require_workspace_write | — | RepositorySyncResponse |
| GET | `/v1/repositories/{source_id}/files` | auth | require_workspace_read | — | Vec<RepoFile |
| GET | `/v1/repositories/{source_id}/detail` | auth | require_workspace_read | — | RepositoryDetailResponse |
| POST | `/v1/repositories/{source_id}/summary` | auth | require_workspace_write, authorize_ai_use | — | RepoSummary |

## Jobs and events

| Method | Path | Authentication | Guards | Request body | Response |
|---|---|---|---|---|---|
| GET | `/v1/workspaces/{workspace_id}/jobs` | auth | require_workspace_read | — | Vec<IndexingJobStatus |
| GET | `/v1/jobs/{job_id}` | auth | require_workspace_read | — | IndexingJobStatus |
| POST | `/v1/jobs/{job_id}/cancel` | auth | require_workspace_write | — | IndexingJobStatus |
| GET | `/v1/workspaces/{workspace_id}/events` | auth | require_workspace_read | — | — |

## Collaboration

| Method | Path | Authentication | Guards | Request body | Response |
|---|---|---|---|---|---|
| GET | `/v1/workspaces/{workspace_id}/activity` | auth | require_workspace_admin | — | Vec<WorkspaceActivityEvent |
| GET | `/v1/workspaces/{workspace_id}/activity/calendar` | auth | require_workspace_admin | — | WorkspaceActivityCalendarResponse |
| GET | `/v1/workspaces/{workspace_id}/tasks` | auth | require_workspace_read | — | Vec<CollaborationTask |
| POST | `/v1/workspaces/{workspace_id}/tasks` | auth | require_workspace_write | SaveCollaborationTaskRequest | CollaborationTask |
| GET | `/v1/workspaces/{workspace_id}/saved-searches` | auth | require_workspace_read | — | Vec<SavedSearch |
| POST | `/v1/workspaces/{workspace_id}/saved-searches` | auth | require_workspace_write | SaveSearchRequest | SavedSearch |
| GET | `/v1/tasks/{task_id}` | auth | require_workspace_read | — | CollaborationTask |
| PUT | `/v1/tasks/{task_id}` | auth | require_workspace_write | SaveCollaborationTaskRequest | CollaborationTask |
| DELETE | `/v1/tasks/{task_id}` | auth | require_workspace_write | — | status |
| GET | `/v1/tasks/{task_id}/checklist` | auth | require_workspace_read | — | Vec<TaskChecklistItem |
| POST | `/v1/tasks/{task_id}/checklist` | auth | require_workspace_write | CreateChecklistItemRequest | TaskChecklistItem |
| PUT | `/v1/task-checklist/{item_id}` | auth | require_workspace_write | ToggleChecklistItemRequest | TaskChecklistItem |
| DELETE | `/v1/task-checklist/{item_id}` | auth | require_workspace_write | — | status |
| DELETE | `/v1/saved-searches/{search_id}` | auth | require_workspace_write | — | status |
| GET | `/v1/workspaces/{workspace_id}/members` | auth | require_workspace_admin | — | Vec<WorkspaceMember |
| PUT | `/v1/workspaces/{workspace_id}/members` | auth | require_workspace_admin | UpsertWorkspaceMemberRequest | WorkspaceMember |
| GET | `/v1/workspaces/{workspace_id}/invitations` | auth | require_workspace_admin | — | Vec<Invitation |
| POST | `/v1/workspaces/{workspace_id}/invitations` | auth | require_workspace_admin | CreateInvitationRequest | CreatedInvitation |
| DELETE | `/v1/workspaces/{workspace_id}/members/{user_id}` | auth | require_workspace_admin | — | status |
| PUT | `/v1/comments/{comment_id}` | auth | require_workspace_write | SaveArtifactCommentRequest | ArtifactComment |
| DELETE | `/v1/comments/{comment_id}` | auth | require_workspace_write | — | status |

## Workspace settings and AI

| Method | Path | Authentication | Guards | Request body | Response |
|---|---|---|---|---|---|
| GET | `/v1/workspaces/{workspace_id}/overview` | auth | require_workspace_read | — | WorkspaceOverview |
| GET | `/v1/workspaces/{workspace_id}/metrics` | auth | require_workspace_read | — | WorkspaceMetricsResponse |
| GET | `/v1/workspaces/{workspace_id}/capabilities` | auth | require_workspace_read | — | WorkspaceCapabilities |
| GET | `/v1/workspaces/{workspace_id}/ai-providers` | auth | require_workspace_admin | — | Vec<SharedAiProviderSettings |
| PUT | `/v1/workspaces/{workspace_id}/ai-providers` | auth | require_workspace_admin | SaveAiProviderRequest | SharedAiProviderSettings |
| GET | `/v1/workspaces/{workspace_id}/ai-policy` | auth | require_workspace_read | — | AiPolicyResponse |
| PUT | `/v1/workspaces/{workspace_id}/ai-policy` | auth | require_workspace_admin | UpdateAiPolicyRequest | AiPolicyResponse |
| POST | `/v1/workspaces/{workspace_id}/ai-providers/{provider_id}/test` | auth | require_workspace_admin | — | ProviderTestResult |
| POST | `/v1/workspaces/{workspace_id}/transfer-ownership` | session | require_workspace_owner | TransferOwnershipRequest | status |
