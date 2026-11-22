import { useState } from "react";
import { Rows as LayoutList } from "@phosphor-icons/react";
import { saveSharedWorkspaceTemplate } from "../lib/sharedApi";
import type { WorkspaceTemplateView } from "../types";
import { ADMIN_WORKSPACE_SECTIONS, WORKSPACE_SECTIONS, type WorkspaceSection } from "../web/constants";
import { apiMessage } from "../web/format";
import { adjustmentsFrom, SECTION_NAMES } from "../web/templatePreview";
import { useTemplate, useTemplateCatalogue, useTemplateDetail } from "../web/workspace/data";
import { TemplatePicker } from "./TemplatePicker";
import { Collapse } from "./ui/collapse";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import { useConfirm } from "./ui/confirm-dialog";
import { Dropdown } from "./ui/dropdown";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";

/**
 * Settings › Template (V0.2.177), for owners and administrators: which template the workspace uses, a preview of what
 * another would make of it, the few adjustments that can be saved on top (the section it opens on, what sits under
 * More, the suggested questions) and a reset to the template's own values. Choosing or adjusting a template never
 * moves, creates or deletes evidence, and it changes nothing about who may do what.
 */
export function TemplateSettings({ accessToken, embedded = false, workspaceId }: { accessToken: string; /** Drawn as a part of the Customization section: a plain title, no panel of its own. */ embedded?: boolean; workspaceId: string }) {
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
    <section className={embedded ? "shared-settings-group shared-settings-part" : "shared-settings-group"}>
      {embedded
        ? <div className="shared-part-heading"><h3 className="shared-part-title">Template</h3>{current.source === "fallback" ? null : <span>Version {current.template.version}</span>}</div>
        : <PanelHeading description={current.source === "fallback" ? undefined : `Version ${current.template.version}`} icon={LayoutList} title="Template" />}
      {current.source === "fallback" ? <p className="shared-muted-copy">{current.reason} General is in use until a template is chosen.</p> : null}
      <TemplatePicker disabled={busy} name={`settings-template-${workspaceId}`} onChange={setChoice} templates={catalogue} value={chosenId} activeId={currentId} hideLegend={embedded} />
      {(() => {
        // Nothing to apply while the template in use is the one chosen: the button stays, grey and disabled.
        const applicable = comparing || current.source !== "workspace";
        return <Button busy={busy} className="rm-template-use" disabled={busy || !applicable || (comparing && !detail)} onClick={() => void apply()} type="button" variant={applicable ? "main" : "secondary"}>Use this template</Button>;
      })()}
      <Collapse open={current.source === "workspace" && !comparing && Boolean(base)}>
        {current.source === "workspace" && base ? <Adjustments
        adjusted={adjusted}
        base={base}
        busy={busy}
        current={current}
        key={JSON.stringify([current.sections, current.assistant.suggestions])}
        onReset={() => void save(currentId, {}, "The adjustments were removed; the template's own values are back.")}
        onSave={(overrides) => void save(currentId, overrides, Object.keys(overrides).length ? "The adjustments were saved." : "No adjustment differs from the template.")}
      /> : null}
      </Collapse>
    </section>
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
  const name = (section: WorkspaceSection) => current.labels.sections[section] ?? SECTION_NAMES[section];
  const toggle = (section: string, keep: boolean) => setMore((list) => keep ? [...list.filter((entry) => entry !== section), section] : list.filter((entry) => entry !== section));
  return (
    <fieldset className="rm-template-adjust" disabled={busy}>
      <h3 className="shared-part-title">Adjust this workspace</h3>
      <p className="shared-muted-copy">Only what you change here is saved on top of the template; everything else keeps following it.</p>
      <label className="rm-template-adjust-field">Opens on
        <Dropdown aria-label="The section the workspace opens on" onValueChange={setLanding} options={WORKSPACE_SECTIONS.filter((section) => !ADMIN_WORKSPACE_SECTIONS.has(section)).map((section) => ({ label: name(section), value: section }))} value={landing} />
      </label>
      <p className="shared-muted-copy">Tick the pages that should leave the row of tabs and sit under <strong>More</strong> instead.</p>
      <div aria-label="Sections kept under More" className="rm-template-adjust-more" role="group">
        {WORKSPACE_SECTIONS.map((section) => <Checkbox checked={more.includes(section)} key={section} label={name(section)} onCheckedChange={(keep) => toggle(section, keep)} />)}
      </div>
      <div className="rm-template-adjust-actions">
        <Button busy={busy} disabled={busy} onClick={() => onSave(adjustmentsFrom(base, { landing, more, suggestions: current.assistant.suggestions }))} type="button" variant="main">Save adjustments</Button>
        <Button disabled={busy || !adjusted} onClick={onReset} type="button" variant="secondary">Reset to template</Button>
      </div>
    </fieldset>
  );
}
