import { useState } from "react";
import { Rows as LayoutList } from "@phosphor-icons/react";
import { saveSharedWorkspaceTemplate } from "../lib/sharedApi";
import { apiMessage } from "../web/format";
import { useTemplate, useTemplateCatalogue, useTemplateDetail } from "../web/workspace/data";
import { TemplatePicker } from "./TemplatePicker";
import { Button } from "./ui/button";
import { useConfirm } from "./ui/confirm-dialog";
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
  if (!current) return null;
  const chosenName = catalogue.find((template) => template.id === chosenId)?.name ?? chosenId;

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
    </section>
  );
}
