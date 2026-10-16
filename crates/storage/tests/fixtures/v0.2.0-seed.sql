-- A small environment as RepoMemo 0.2.0 stores it (the schema of migrations 0001 to 0024), loaded by
-- `an_environment_made_by_v0_2_0_upgrades_in_place_and_keeps_its_data` before the current
-- migrations run. It holds rows the way 0.2.0 wrote them and none of a later table.
-- The text of the two stored files is in the test; the hashes below are their SHA-256.

INSERT INTO users (id, email, display_name, password_hash, created_at, updated_at, session_version, is_system_admin, is_app_admin)
VALUES
  ('u-ada', 'ada@example.com', 'Ada', '$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0$aGFzaGhhc2hoYXNoaGFzaGhhc2hoYXNoaGFzaA', '2026-09-01T10:00:00+00:00', '2026-09-01T10:00:00+00:00', 3, 1, 0),
  ('u-grace', 'grace@example.com', 'Grace', '$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0$aGFzaGhhc2hoYXNoaGFzaGhhc2hoYXNoaGFzaA', '2026-09-02T10:00:00+00:00', '2026-09-02T10:00:00+00:00', 0, 0, 0);

INSERT INTO organizations (id, name, created_at, updated_at)
VALUES ('o-acme', 'Acme', '2026-09-01T10:05:00+00:00', '2026-09-01T10:05:00+00:00');

INSERT INTO organization_memberships (organization_id, user_id, role, created_at)
VALUES ('o-acme', 'u-ada', 'owner', '2026-09-01T10:05:00+00:00'),
       ('o-acme', 'u-grace', 'member', '2026-09-02T10:05:00+00:00');

INSERT INTO workspaces (id, name, created_at, updated_at, settings_json)
VALUES ('w-atlas', 'Atlas', '2026-09-03T09:00:00+00:00', '2026-09-10T09:00:00+00:00', '{}');

INSERT INTO workspace_organizations (workspace_id, organization_id) VALUES ('w-atlas', 'o-acme');

INSERT INTO workspace_memberships (workspace_id, user_id, role, created_at, updated_at)
VALUES ('w-atlas', 'u-ada', 'owner', '2026-09-03T09:00:00+00:00', '2026-09-03T09:00:00+00:00'),
       ('w-atlas', 'u-grace', 'member', '2026-09-04T09:00:00+00:00', '2026-09-04T09:00:00+00:00');

INSERT INTO sources (id, workspace_id, type, name, status, created_at, updated_at)
VALUES ('s-uploads', 'w-atlas', 'upload', 'Uploads', 'ready', '2026-09-03T09:10:00+00:00', '2026-09-03T09:10:00+00:00');

INSERT INTO folders (id, workspace_id, parent_id, name, created_by, created_at)
VALUES ('f-policies', 'w-atlas', NULL, 'Policies', 'u-ada', '2026-09-03T09:12:00+00:00');

INSERT INTO blobs (hash, storage_uri, size_bytes, mime_type, created_at)
VALUES ('25b9033a79705adbe5e5e258fee1e70adab08041373f9b0078fc0b9c3ffbe561', 'blobs/25/b9/25b9033a79705adbe5e5e258fee1e70adab08041373f9b0078fc0b9c3ffbe561', 52, 'text/markdown', '2026-09-03T09:15:00+00:00'),
       ('5eb51d20b9012cdff150b9825024a9ae9514028e20e158524e8e0861020df504', 'blobs/5e/b5/5eb51d20b9012cdff150b9825024a9ae9514028e20e158524e8e0861020df504', 42, 'text/markdown', '2026-09-03T09:16:00+00:00');

INSERT INTO artifacts (id, workspace_id, source_id, type, title, path, content_hash, mime_type, language, size_bytes, created_at, updated_at, indexed_at, metadata_json, index_version, folder_id)
VALUES
  ('a-refunds', 'w-atlas', 's-uploads', 'markdown_doc', 'Refund policy', 'refund-policy.md', '25b9033a79705adbe5e5e258fee1e70adab08041373f9b0078fc0b9c3ffbe561', 'text/markdown', 'Markdown', 52, '2026-09-03T09:15:00+00:00', '2026-09-03T09:15:00+00:00', '2026-09-03T09:16:00+00:00', '{}', 1, 'f-policies'),
  ('a-shipping', 'w-atlas', 's-uploads', 'markdown_doc', 'Shipping notes', 'shipping-notes.md', '5eb51d20b9012cdff150b9825024a9ae9514028e20e158524e8e0861020df504', 'text/markdown', 'Markdown', 42, '2026-09-03T09:16:00+00:00', '2026-09-05T09:16:00+00:00', '2026-09-05T09:17:00+00:00', '{}', 1, NULL);

INSERT INTO chunks (id, artifact_id, workspace_id, chunk_index, text, token_count, start_line, end_line, heading_path, content_hash)
VALUES ('c-shipping-0', 'a-shipping', 'w-atlas', 0, 'Parcels leave the warehouse every weekday.', 7, 1, 1, NULL, '5eb51d20b9012cdff150b9825024a9ae9514028e20e158524e8e0861020df504'),
       ('c-refunds-0', 'a-refunds', 'w-atlas', 0, 'Refunds are issued within fourteen days of purchase.', 9, 1, 1, NULL, '25b9033a79705adbe5e5e258fee1e70adab08041373f9b0078fc0b9c3ffbe561');

INSERT INTO memory_cards (id, workspace_id, title, body_markdown, source, confidence, created_at, updated_at, metadata_json)
VALUES ('m-dispatch', 'w-atlas', 'Dispatch rhythm', 'Parcels leave every weekday.', 'manual', NULL, '2026-09-06T09:00:00+00:00', '2026-09-06T09:00:00+00:00', '{}');

INSERT INTO links (id, workspace_id, from_id, from_type, to_id, to_type, relation_type, confidence, created_by, metadata_json, created_at)
VALUES ('l-passage', 'w-atlas', 'm-dispatch', 'memory_card', 'c-shipping-0', 'chunk', 'cites', NULL, 'user', '{}', '2026-09-06T09:00:00+00:00'),
       ('l-file', 'w-atlas', 'm-dispatch', 'memory_card', 'a-refunds', 'artifact', 'cites', NULL, 'user', '{}', '2026-09-06T09:00:00+00:00');

INSERT INTO artifact_lifecycle (artifact_id, workspace_id, status, owner_user_id, review_note, reviewed_by_user_id, reviewed_at, created_at, updated_at)
VALUES ('a-refunds', 'w-atlas', 'verified', 'u-ada', 'Checked against the policy.', 'u-ada', '2026-09-07T09:00:00+00:00', '2026-09-03T09:15:00+00:00', '2026-09-07T09:00:00+00:00');

INSERT INTO artifact_lifecycle_events (id, artifact_id, actor_user_id, action, detail, created_at)
VALUES ('e-verified', 'a-refunds', 'u-ada', 'status_changed', 'active to verified', '2026-09-07T09:00:00+00:00');

INSERT INTO workspace_tasks (id, workspace_id, title, description, status, priority, assignee_user_id, created_by_user_id, artifact_id, due_at, created_at, updated_at)
VALUES ('t-check', 'w-atlas', 'Check the policy', 'Read it twice.', 'open', 'high', 'u-grace', 'u-ada', 'a-refunds', NULL, '2026-09-08T09:00:00+00:00', '2026-09-08T09:00:00+00:00');

INSERT INTO workspace_task_checklist_items (id, task_id, workspace_id, created_by_user_id, body, position, created_at, updated_at)
VALUES ('k-read', 't-check', 'w-atlas', 'u-ada', 'Read it twice', 0, '2026-09-08T09:01:00+00:00', '2026-09-08T09:01:00+00:00');

INSERT INTO artifact_comments (id, workspace_id, artifact_id, author_user_id, body, created_at, updated_at)
VALUES ('n-looks-right', 'w-atlas', 'a-refunds', 'u-grace', 'Looks right to me.', '2026-09-09T09:00:00+00:00', '2026-09-09T09:00:00+00:00');

INSERT INTO workspace_activity (id, workspace_id, actor_user_id, action, subject_type, subject_id, summary, created_at)
VALUES ('v-uploaded', 'w-atlas', 'u-ada', 'evidence_uploaded', 'artifact', 'a-refunds', 'Uploaded refund-policy.md.', '2026-09-03T09:15:00+00:00');

INSERT INTO system_settings (key, value_json, updated_at, updated_by_user_id)
VALUES ('log_http_level', '"debug"', '2026-09-12T09:00:00+00:00', 'u-ada');
