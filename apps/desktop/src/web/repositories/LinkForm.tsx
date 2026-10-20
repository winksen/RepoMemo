import { IconCircleCheck as CircleCheck, IconGitBranch as GitBranch, IconGitCommit as GitCommit } from "@tabler/icons-react";
import type { RepoAccessCheck } from "../../types";
import { formatTime } from "../format";
import { Field, FieldRow } from "../../components/ui/field";
import { Input } from "../../components/ui/input";
import { Textarea } from "../../components/ui/textarea";
import { ICON_SIZE } from "../../components/ui/icon";
import { RepoForm, isSshLink, isRemoteLink } from "./shared";

export function RepositoryCheckResult({ check }: { check: RepoAccessCheck }) {
  return <div className="rm-repo-check">
    <strong><CircleCheck size={ICON_SIZE.md} /> {check.remote ? "The host answered: the server can reach this repository" : "The server can read this repository"}</strong>
    <dl className="rm-repo-facts">
      <div><dt>Repository</dt><dd>{check.name} <code className="rm-repo-path">{check.root_path}</code></dd></div>
      <div><dt><GitBranch size={ICON_SIZE.sm} /> Branch</dt><dd>{check.commit.branch ?? <span className="rm-map-muted">Detached HEAD</span>}</dd></div>
      {check.remote
        ? <div><dt><GitCommit size={ICON_SIZE.sm} /> Commit</dt><dd><code>{check.commit.sha.slice(0, 7)}</code> <span className="rm-map-muted">· the files are counted after the first sync, which clones the repository</span></dd></div>
        : <>
          <div><dt><GitCommit size={ICON_SIZE.sm} /> Commit</dt><dd><code>{check.commit.sha.slice(0, 7)}</code> {check.commit.summary} <span className="rm-map-muted">· {check.commit.author_name} · {formatTime(check.commit.committed_at)}</span></dd></div>
          <div><dt>Files</dt><dd>About {check.indexable_files.toLocaleString()} of {check.tracked_files.toLocaleString()} committed files will be indexed</dd></div>
        </>}
    </dl>
    {check.already_connected ? <span className="rm-map-muted">This repository is already linked to the workspace.</span> : null}
  </div>;
}

export function RepositoryFields({
  form,
  onChange,
  showLink,
  remote = false,
  replacingToken = false,
}: {
  form: RepoForm;
  onChange: (form: RepoForm) => void;
  showLink: boolean;
  /** The repository is a remote one (an existing link, in the edit form). */
  remote?: boolean;
  replacingToken?: boolean;
}) {
  const set = (key: keyof RepoForm) => (event: { target: { value: string } }) => onChange({ ...form, [key]: event.target.value });
  const sshLink = showLink ? isSshLink(form.link) : replacingToken && isSshLink(form.link);
  const asksForToken = showLink ? isRemoteLink(form.link) && !sshLink : remote && !sshLink;
  return <>
    {showLink ? <Field hint="A folder on the server, anywhere inside the repository, or the https address of a repository on a host the server operator allows." label="Repository link">
      <Input onChange={set("link")} placeholder="C:\code\my-repo, /srv/code/my-repo or https://github.com/acme/app.git" value={form.link} />
    </Field> : null}
    {sshLink ? <>
      <Field hint="A read-only deploy key without a passphrase. It is stored encrypted and never shown again." label={replacingToken ? "New deploy key" : "Deploy key"}>
        <Textarea autoComplete="off" onChange={set("deployKey")} placeholder={replacingToken ? "Leave empty to keep the current key" : "-----BEGIN OPENSSH PRIVATE KEY-----"} rows={4} value={form.deployKey} />
      </Field>
      <Field hint={<>The host's public key, as printed by <code>ssh-keyscan -t ed25519 host</code>. The connection only goes through to a host that presents exactly this key.</>} label="Host key">
        <Textarea autoComplete="off" onChange={set("hostKey")} placeholder="ssh-ed25519 AAAA…" rows={2} value={form.hostKey} />
      </Field>
    </> : null}
    {asksForToken ? <FieldRow>
      <Field hint="A read-only token. It is stored encrypted and never shown again." label={replacingToken ? "New access token" : "Access token"}>
        <Input autoComplete="off" onChange={set("token")} placeholder={replacingToken ? "Leave empty to keep the current token" : "Only for a private repository"} type="password" value={form.token} />
      </Field>
      <Field hint={<>Optional; <code>oauth2</code> for GitLab.</>} label="Token account name">
        <Input autoComplete="off" onChange={set("tokenUsername")} placeholder="x-access-token" value={form.tokenUsername} />
      </Field>
    </FieldRow> : null}
    <FieldRow>
      <Field label="Name"><Input onChange={set("name")} placeholder={showLink ? "The folder name" : ""} value={form.name} /></Field>
      <Field label="Branch"><Input onChange={set("branch")} placeholder="The checked-out branch" value={form.branch} /></Field>
    </FieldRow>
    <Field hint="One pattern per line. Leave empty to index everything." label="Include only">
      <Textarea onChange={set("include")} placeholder={"docs/**\nsrc/"} rows={3} value={form.include} />
    </Field>
    <Field hint={<>Added to the defaults: lock files, minified files, <code>node_modules/</code>, <code>vendor/</code>, <code>dist/</code>, <code>build/</code>, <code>target/</code>, images and files over 1 MB.</>} label="Exclude">
      <Textarea onChange={set("exclude")} placeholder={"**/*.test.ts\nfixtures/"} rows={3} value={form.exclude} />
    </Field>
  </>;
}
