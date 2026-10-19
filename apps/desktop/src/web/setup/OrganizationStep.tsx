import { useState } from "react";
import type { FormEvent } from "react";
import { IconArrowLeft as ArrowLeft, IconArrowRight as ArrowRight, IconBuilding as Building, IconCircleCheck as CircleCheck, IconLoader2 as Loader } from "@tabler/icons-react";
import { createSharedOrganization, createSharedWorkspace } from "../../lib/sharedApi";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage } from "../format";
import { StepHeading } from "./StepFrame";

export function OrganizationStep({ accessToken, created, onBack, onCreated, onNext }: {
  accessToken: string;
  created: { organization: string; workspace: string | null } | null;
  onBack: () => void;
  onCreated: (created: { organization: string; workspace: string | null }) => void;
  onNext: () => void;
}) {
  const [organization, setOrganization] = useState("");
  const [workspace, setWorkspace] = useState("");
  const [isBusy, setIsBusy] = useState(false);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsBusy(true);
    try {
      const createdOrganization = await createSharedOrganization(accessToken, organization.trim());
      const createdWorkspace = workspace.trim() ? await createSharedWorkspace(accessToken, createdOrganization.id, workspace.trim()) : null;
      onCreated({ organization: createdOrganization.name, workspace: createdWorkspace?.workspace.name ?? null });
      onNext();
    } catch (error) { showToast("error", apiMessage(error, "The organization could not be created.")); }
    finally { setIsBusy(false); }
  }
  return <>
    <StepHeading eyebrow="Step 6 of 7" title="Create a first workspace">
      Optional. An organization is a team or company; a workspace holds one body of knowledge, such as a project or a service. You own what you create here and can invite people afterwards.
    </StepHeading>
    {created ? <p className="rm-setup-created"><CircleCheck size={ICON_SIZE.md} /> Created {created.organization}{created.workspace ? ` with the workspace ${created.workspace}` : ""}.</p> : null}
    {created ? <div className="rm-setup-actions"><Button onClick={onBack} type="button" variant="secondary"><ArrowLeft size={ICON_SIZE.md} /> Back</Button><Button onClick={onNext} type="button" variant="main"><ArrowRight size={ICON_SIZE.md} /> Continue</Button></div> : <form className="shared-setup-form" onSubmit={submit}>
      <label>Organization name<Input maxLength={120} onChange={(event) => setOrganization(event.target.value)} placeholder="Acme Engineering" required value={organization} /></label>
      <label>Workspace name (optional)<Input maxLength={120} onChange={(event) => setWorkspace(event.target.value)} placeholder="Payments service" value={workspace} /></label>
      <div className="rm-setup-actions">
        <Button disabled={isBusy} onClick={onBack} type="button" variant="secondary"><ArrowLeft size={ICON_SIZE.md} /> Back</Button>
        <span className="rm-setup-actions-end"><Button disabled={isBusy} onClick={onNext} type="button" variant="secondary">Skip</Button><Button disabled={isBusy || !organization.trim()} type="submit" variant="main">{isBusy ? <Loader className="spin" size={ICON_SIZE.md} /> : <Building size={ICON_SIZE.md} />} Create and continue</Button></span>
      </div>
    </form>}
  </>;
}
