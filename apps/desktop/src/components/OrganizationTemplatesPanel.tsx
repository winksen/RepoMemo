import { useCallback, useEffect, useState } from "react";
import { SquaresFour as Layout } from "@phosphor-icons/react";
import { getSharedOrganizationTemplateVersions, listSharedOrganizationTemplates, saveSharedOrganizationTemplate, type OrganizationTemplate } from "../lib/sharedApi";
import { Button } from "./ui/button";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { PanelHeading } from "./ui/panel-heading";
import { Textarea } from "./ui/textarea";
import { showToast } from "./ui/toast";
import { apiMessage } from "../web/format";
import { formatDateValue } from "../lib/locale";

const when = (value: string) => formatDateValue(value);

/**
 * The organization's own workspace templates (V0.6.23). An administrator saves a template file (the one a workspace's setup
 * is exported as, in Settings › Template), each save is the next version, and every workspace of the organization can preview
 * and adopt it from Settings › Template. A template arranges and names what a workspace shows; it never changes who can see or
 * do what, and it never moves content. Everyone in the organization sees the list; only an administrator edits.
 */
export function OrganizationTemplatesPanel({ accessToken, canManage, organizationId }: { accessToken: string; canManage: boolean; organizationId: string }) {
  const [templates, setTemplates] = useState<OrganizationTemplate[] | null>(null);
  const [definition, setDefinition] = useState("");
  const [note, setNote] = useState("");
  const [isSaving, setIsSaving] = useState(false);

  const load = useCallback(async () => {
    try { setTemplates(await listSharedOrganizationTemplates(accessToken, organizationId)); } catch (error) { setTemplates([]); showToast("error", apiMessage(error, "The templates could not be read.")); }
  }, [accessToken, organizationId]);
  useEffect(() => { void load(); }, [load]);

  async function edit(template: OrganizationTemplate) {
    try {
      const versions = await getSharedOrganizationTemplateVersions(accessToken, organizationId, template.id);
      const newest = versions[0];
      if (!newest) return;
      // The next version: the file with its version raised by one, ready to change and save.
      const file = JSON.parse(newest.definition) as { meta: { version: number } };
      file.meta.version = newest.version + 1;
      setDefinition(JSON.stringify(file, null, 2));
      setNote("");
    } catch (error) { showToast("error", apiMessage(error, "That template could not be opened.")); }
  }

  async function save() {
    setIsSaving(true);
    try {
      const saved = await saveSharedOrganizationTemplate(accessToken, organizationId, definition, note);
      showToast("success", `Saved ${saved.name}, version ${saved.version}. Workspaces of this organization can adopt it now.`);
      setDefinition("");
      setNote("");
      await load();
    } catch (error) { showToast("error", apiMessage(error, "The template could not be saved.")); } finally { setIsSaving(false); }
  }

  return (
    <section aria-labelledby="organization-templates-title" className="shared-settings-group">
      <PanelHeading description={canManage ? "administrators save them" : "chosen in Settings › Template"} icon={Layout} id="organization-templates-title" title="Organization templates" />
      <p className="shared-muted-copy">A template arranges the tabs, names and Overview a workspace starts with. It never changes who can see or do anything and never moves content. Saving a new version updates the workspaces that use it; each one keeps its own adjustments.</p>
      {templates === null ? null : templates.length === 0 ? <p className="shared-muted-copy">This organization has no templates of its own yet.</p> : <ul aria-label="Organization templates" className="shared-version-list">
        {templates.map((template) => <li key={template.id}>
          <div><strong>{template.name}</strong><small>{template.id} · version {template.version} · saved {when(template.saved_at)}{template.saved_by ? ` by ${template.saved_by.display_name}` : ""}{template.note ? ` · ${template.note}` : ""}</small></div>
          {canManage ? <Button onClick={() => void edit(template)} type="button" variant="secondary">New version</Button> : null}
        </li>)}
      </ul>}
      {canManage ? <form className="rm-organization-template-form" onSubmit={(event) => { event.preventDefault(); void save(); }}>
        <Field hint="Paste a template file, for example one exported from a workspace in Settings › Template. Its version must be one more than the newest saved (1 for a new template)." label="Template file (JSON)"><Textarea onChange={(event) => setDefinition(event.target.value)} rows={10} spellCheck={false} value={definition} /></Field>
        <Field label="Note for the history"><Input maxLength={200} onChange={(event) => setNote(event.target.value)} value={note} /></Field>
        <div><Button busy={isSaving} disabled={!definition.trim()} type="submit" variant="main">Save template</Button></div>
      </form> : null}
    </section>
  );
}
