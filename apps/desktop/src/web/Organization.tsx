import { type FormEvent, type ReactNode, useEffect, useState } from "react";
import { IconBook2 as Book, IconBuildingCommunity as Building, IconChartDots3 as MapIcon, IconChecklist as Checklist, IconEyeCheck as ReviewIcon, IconFileText as FileText, IconGitBranch as RepositoryIcon, IconLayoutDashboard as Dashboard, IconPlus as Plus, IconSearch as Search, IconSettings as Settings, IconStack2 as Layers, IconStethoscope as HealthIcon, IconTrash as Trash, IconUsers as Users } from "@tabler/icons-react";
import { SharedApiError, listSharedOrganizationMembers, removeSharedOrganizationMember, transferSharedOrganizationOwnership, updateSharedOrganization, upsertSharedOrganizationMember } from "../lib/sharedApi";
import { type Organization, type OrganizationMember, type OrganizationRole, type SharedWorkspace } from "../types";
import { UserAvatar } from "../components/UserAvatar";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Field } from "../components/ui/field";
import { LoadingState } from "../components/ui/loading-state";
import { PanelHeading } from "../components/ui/panel-heading";
import { useConfirm } from "../components/ui/confirm-dialog";
import { Dropdown } from "../components/ui/dropdown";
import { IconButton } from "../components/ui/icon-button";
import { Input } from "../components/ui/input";
import { Toast, showToast } from "../components/ui/toast";
import { InviteMembers } from "../components/InviteMembers";
import { ADMIN_WORKSPACE_SECTIONS, type WorkspaceSection } from "./constants";
import { apiMessage } from "./format";
import { navigate } from "./router";
import { useOrganizationNavigation } from "./navigation";
import { ICON_SIZE } from "../components/ui/icon";

export function OrganizationAdministration({
  accessToken,
  onOrganizationUpdated,
  organization,
}: {
  accessToken: string;
  onOrganizationUpdated: (organization: Organization) => void;
  organization: Organization;
}) {
  const confirm = useConfirm();
  const [members, setMembers] = useState<OrganizationMember[]>([]);
  const [organizationName, setOrganizationName] = useState(organization.name);
  const [memberEmail, setMemberEmail] = useState("");
  const [memberRole, setMemberRole] = useState<OrganizationRole>("member");
  const [inviteEmail, setInviteEmail] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const canRename = organization.role === "owner";
  const canAssignAdmin = organization.role === "owner";

  async function load() {
    setIsLoading(true); setError(null);
    try { setMembers(await listSharedOrganizationMembers(accessToken, organization.id)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsLoading(false); }
  }

  useEffect(() => { setOrganizationName(organization.name); void load(); }, [accessToken, organization.id, organization.name]);

  async function saveOrganization(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSaving(true); setError(null);
    try { onOrganizationUpdated(await updateSharedOrganization(accessToken, organization.id, organizationName)); showToast("success", "Organization renamed."); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSaving(false); }
  }

  async function saveMember(email: string, role: OrganizationRole) {
    setIsSaving(true); setError(null);
    try {
      const member = await upsertSharedOrganizationMember(accessToken, organization.id, { email, role });
      setMembers((current) => current.some((entry) => entry.user.id === member.user.id) ? current.map((entry) => entry.user.id === member.user.id ? member : entry) : [...current, member]);
      setMemberEmail("");
      showToast("success", `${member.user.display_name} is now ${role} in the organization.`);
    } catch (requestError) {
      // Not in the organization yet (with or without an account): they join by invitation.
      if (requestError instanceof SharedApiError && requestError.status === 409) {
        setInviteEmail(email.trim());
        showToast("warning", `${email.trim()} is not in this organization yet. Create an invite link for them below.`);
      } else setError(apiMessage(requestError));
    } finally { setIsSaving(false); }
  }

  async function makeOwner(member: OrganizationMember) {
    if (!(await confirm({ action: "Hand over", description: "You stay on as an administrator and cannot take it back yourself.", title: `Hand this organization to ${member.user.display_name}?` }))) return;
    setIsSaving(true); setError(null);
    try {
      await transferSharedOrganizationOwnership(accessToken, organization.id, member.user.id);
      window.location.reload();
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSaving(false); }
  }

  async function removeMember(member: OrganizationMember) {
    setIsSaving(true); setError(null);
    try { await removeSharedOrganizationMember(accessToken, organization.id, member.user.id); setMembers((current) => current.filter((entry) => entry.user.id !== member.user.id)); showToast("success", `${member.user.display_name} was removed from the organization.`); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSaving(false); }
  }

  return <section className="shared-organization-admin" aria-labelledby="organization-admin-title">
    <PanelHeading description={organization.role} icon={Building} id="organization-admin-title" title="Organization administration" />
    <p className="shared-muted-copy">Organization members inherit access to every workspace. Their organization role controls the level of that access.</p>
    {canRename ? <form className="shared-organization-rename" onSubmit={saveOrganization}><Field label="Organization name" required><Input onChange={(event) => setOrganizationName(event.target.value)} value={organizationName} /></Field><Button busy={isSaving} disabled={organizationName.trim() === organization.name} type="submit" variant="secondary">Save name</Button></form> : null}
    <form className="shared-organization-member-form" onSubmit={(event) => { event.preventDefault(); void saveMember(memberEmail, memberRole); }}><Field label="Member email" required><Input onChange={(event) => setMemberEmail(event.target.value)} placeholder="person@example.com" type="email" value={memberEmail} /></Field><Field label="Organization role">{(control) => <Dropdown id={control.id} onValueChange={(value) => setMemberRole(value as OrganizationRole)} options={[{ label: "Member", value: "member" }, ...(canAssignAdmin ? [{ label: "Admin", value: "admin" }] : [])]} value={memberRole} />}</Field><Button busy={isSaving} type="submit" variant="secondary"><Users size={ICON_SIZE.md} /> Add member</Button></form>
    <Toast kind="error" message={error} />
    <div className="shared-organization-member-list" aria-busy={isLoading}>{isLoading ? <LoadingState label="Loading organization members" /> : members.map((member) => {
      const isOwner = member.role === "owner";
      const protectedMember = isOwner || (organization.role === "admin" && member.role === "admin");
      const email = member.user.email ?? "";
      return <article key={member.user.id}><div className="shared-person"><UserAvatar name={member.user.display_name} size={32} userId={member.user.id} /><div><strong>{member.user.display_name}</strong><span>{email}</span></div></div><div className="shared-organization-member-actions">{organization.role === "owner" && !isOwner ? <Button disabled={isSaving} onClick={() => void makeOwner(member)} type="button" variant="secondary">Make owner</Button> : null}{isOwner ? <Badge>Owner</Badge> : <Dropdown aria-label={`Role for ${member.user.display_name}`} disabled={protectedMember || isSaving} onValueChange={(value) => void saveMember(email, value as OrganizationRole)} options={[{ label: "Member", value: "member" }, ...(canAssignAdmin ? [{ label: "Admin", value: "admin" }] : [])]} value={member.role} />}{!protectedMember ? <IconButton disabled={isSaving} label={`Remove ${member.user.display_name} from ${organization.name}`} onClick={() => void removeMember(member)}><Trash size={ICON_SIZE.sm} /></IconButton> : null}</div></article>;
    })}</div>
    <InviteMembers accessToken={accessToken} initialEmail={inviteEmail} key={`invite-${organization.id}`} roleOptions={[{ label: "Member", value: "member" }, ...(canAssignAdmin ? [{ label: "Admin", value: "admin" }] : [])]} targetId={organization.id} targetLabel="organization" targetType="organization" />
  </section>;
}

export function OrganizationRail({
  organizations,
  organizationId,
  showSelection = true,
  workspaceId,
  workspaces,
}: {
  organizations: Organization[];
  organizationId?: string;
  /** False on pages outside any organization (System), so nothing in the list looks current. */
  showSelection?: boolean;
  workspaceId?: string;
  workspaces?: SharedWorkspace[];
}) {
  const organizationNavigation = useOrganizationNavigation();
  const selectedOrganizationId = showSelection ? organizationId ?? organizationNavigation.activeOrganizationId ?? organizations[0]?.id : undefined;
  const canManageOrganizations = !organizations.length || organizations.some((organization) => organization.role === "owner" || organization.role === "admin");

  return <div className="shared-rail-navigation">
    {workspaces ? <section className="shared-rail-workspaces" aria-label="Workspaces">
      <p className="shared-eyebrow">Workspaces</p>
      {workspaces.length ? <div className="shared-rail-list">{workspaces.map((workspace) => {
        const organization = organizations.find((entry) => entry.id === workspace.organization_id);
        const label = organizations.length > 1 ? `${organization?.name ?? "Organization"} / ${workspace.workspace.name}` : workspace.workspace.name;
        const isCurrent = workspace.workspace.id === workspaceId;
        return <Button aria-current={isCurrent ? "page" : undefined} className={`shared-rail-workspace${isCurrent ? " active" : ""}`} key={workspace.workspace.id} onClick={() => navigate(`/workspaces/${encodeURIComponent(workspace.workspace.id)}/overview`)} type="button" variant="secondary"><Layers size={ICON_SIZE.md} /><span>{label}</span></Button>;
      })}</div> : <p className="shared-rail-empty">No accessible workspaces yet.</p>}
    </section> : null}
    {canManageOrganizations ? <section className="shared-rail-organizations" aria-label="Organization management">
      <p className="shared-eyebrow">Organizations</p>
      {organizations.length ? <div className="shared-rail-list">{organizations.map((organization) => <Button aria-current={organization.id === selectedOrganizationId ? "page" : undefined} className={`shared-rail-organization${organization.id === selectedOrganizationId ? " active" : ""}`} key={organization.id} onClick={() => organizationNavigation.selectOrganization(organization.id)} type="button" variant="secondary"><Building size={ICON_SIZE.md} /><span>{organization.name}</span></Button>)}</div> : <p className="shared-rail-empty">Create an organization to begin.</p>}
      <Button className="shared-organization-create" onClick={organizationNavigation.startCreatingOrganization} type="button" variant="secondary"><Plus size={ICON_SIZE.md} /> New organization</Button>
    </section> : null}
  </div>;
}

export function WorkspaceTopbar({
  activeSection = "overview",
  onNavigate,
  workspace,
}: {
  activeSection?: WorkspaceSection;
  onNavigate?: (section: WorkspaceSection) => void;
  workspace: SharedWorkspace;
}) {
  const isWorkspaceAdmin = workspace.role === "owner" || workspace.role === "admin";
  const workspaceTab = (section: WorkspaceSection, label: string, icon: ReactNode) => {
    const isAdminOnly = ADMIN_WORKSPACE_SECTIONS.has(section);
    if (isAdminOnly && !isWorkspaceAdmin) return null;
    const isActive = activeSection === section;
    return <Button aria-current={isActive ? "page" : undefined} className={isActive ? "active" : ""} disabled={!onNavigate} onClick={() => onNavigate?.(section)} type="button" variant="secondary">{icon}<span className="shared-workspace-tab-label">{label}{isAdminOnly ? <span className="shared-workspace-tab-restricted">Admin</span> : null}</span></Button>;
  };

  return <nav aria-label="Workspace sections" className="shared-workspace-topbar">
    <div className="shared-workspace-tabs">
      {workspaceTab("overview", "Overview", <Dashboard size={ICON_SIZE.md} />)}
      {workspaceTab("evidence", "Evidence", <FileText size={ICON_SIZE.md} />)}
      {workspaceTab("documents", "Documents", <FileText size={ICON_SIZE.md} />)}
      {workspaceTab("repositories", "Repositories", <RepositoryIcon size={ICON_SIZE.md} />)}
      {workspaceTab("retrieval", "Retrieval", <Search size={ICON_SIZE.md} />)}
      {workspaceTab("map", "Map", <MapIcon size={ICON_SIZE.md} />)}
      {workspaceTab("health", "Health", <HealthIcon size={ICON_SIZE.md} />)}
      {workspaceTab("review", "Review", <ReviewIcon size={ICON_SIZE.md} />)}
      {workspaceTab("memory", "Memory", <Book size={ICON_SIZE.md} />)}
      {workspaceTab("tasks", "Tasks", <Checklist size={ICON_SIZE.md} />)}
      {workspaceTab("people", "People & Activity", <Users size={ICON_SIZE.md} />)}
      {workspaceTab("settings", "Settings", <Settings size={ICON_SIZE.md} />)}
    </div>
  </nav>;
}
