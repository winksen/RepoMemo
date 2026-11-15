import { useEffect, useRef, useState } from "react";
import { IconCircle as Circle, IconCircleCheck as CircleCheck, IconListCheck as ListCheck } from "@tabler/icons-react";
import { getSharedOnboarding, setSharedOnboardingDismissed } from "../lib/sharedApi";
import type { WorkspaceOnboarding } from "../types";
import { useLiveRefresh } from "../lib/liveEvents";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage } from "../web/format";
import { t } from "../lib/messages";

/** Activity that can complete a step. */
const CHECKLIST_ACTIONS = /^(artifact_|evidence_|memory_|repository_|member_|invitation_)/;

/**
 * The first-value checklist (V0.6.1): add evidence, make it searchable, save a cited memory, bring in a teammate. A step is
 * done because the thing exists in the workspace, never because it was clicked, and none needs an AI provider. A person
 * can hide the checklist for themselves and bring it back.
 */
export function OnboardingChecklist({ accessToken, workspaceId }: { accessToken: string; workspaceId: string }) {
  const [onboarding, setOnboarding] = useState<WorkspaceOnboarding | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  // Hiding or showing replaces the button that was pressed, so focus goes to its replacement instead of being lost (V0.6.26).
  const refocus = useRef(false);

  async function load() {
    try {
      setOnboarding(await getSharedOnboarding(accessToken, workspaceId));
    } catch (error) {
      showToast("error", apiMessage(error, t("onboarding.readFailed")));
    }
  }

  useEffect(() => { setOnboarding(null); void load(); }, [accessToken, workspaceId]);
  useLiveRefresh(() => void load(), CHECKLIST_ACTIONS);
  useEffect(() => {
    if (!refocus.current) return;
    refocus.current = false;
    document.querySelector<HTMLElement>("[data-onboarding-toggle]")?.focus();
  }, [onboarding?.dismissed]);

  async function setDismissed(dismissed: boolean) {
    setIsSaving(true);
    try {
      await setSharedOnboardingDismissed(accessToken, workspaceId, dismissed);
      refocus.current = true;
      setOnboarding((current) => (current ? { ...current, dismissed } : current));
    } catch (error) {
      showToast("error", apiMessage(error, t("onboarding.saveFailed")));
    } finally {
      setIsSaving(false);
    }
  }

  if (!onboarding) return null;
  const complete = onboarding.done_count === onboarding.total_count;
  if (onboarding.dismissed) {
    return complete ? null : <div className="rm-checklist-collapsed"><Button data-onboarding-toggle disabled={isSaving} onClick={() => void setDismissed(false)} type="button" variant="secondary"><ListCheck size={ICON_SIZE.md} /> {t("onboarding.show", { done: onboarding.done_count, total: onboarding.total_count })}</Button></div>;
  }
  return (
    <section aria-labelledby="onboarding-heading" className="rm-checklist">
      <PanelHeading description={t("onboarding.progress", { done: onboarding.done_count, total: onboarding.total_count })} icon={ListCheck} id="onboarding-heading" title={t("onboarding.title", { template: onboarding.template_name })} />
      <ol className="rm-checklist-steps">
        {onboarding.steps.map((step) => (
          <li data-done={step.done} key={step.id}>
            {step.done ? <CircleCheck aria-hidden size={ICON_SIZE.md} /> : <Circle aria-hidden size={ICON_SIZE.md} />}
            <div><strong>{step.title}</strong><span className="shared-muted-copy">{step.detail}</span></div>
            <span className="rm-checklist-state">{step.done ? t("onboarding.stepDone") : t("onboarding.stepTodo")}</span>
          </li>
        ))}
      </ol>
      <Button data-onboarding-toggle disabled={isSaving} onClick={() => void setDismissed(true)} type="button" variant="secondary">{complete ? t("onboarding.hideComplete") : t("onboarding.hide")}</Button>
    </section>
  );
}
