import { type FormEvent, useMemo } from "react";
import { IconSparkles as Sparkles, IconTimeline as Timeline, IconUsers as Users } from "@tabler/icons-react";
import { SharedApiError, transferSharedWorkspaceOwnership, removeSharedWorkspaceMember, upsertSharedWorkspaceMember } from "../../lib/sharedApi";
import { type WorkspaceMember, type WorkspaceRole } from "../../types";
import { UserAvatar } from "../../components/UserAvatar";
import { Button } from "../../components/ui/button";
import { SegmentedControl } from "../../components/ui/segmented-control";
import { useConfirm } from "../../components/ui/confirm-dialog";
import { Dropdown } from "../../components/ui/dropdown";
import { Input } from "../../components/ui/input";
import { showToast } from "../../components/ui/toast";
import { InviteMembers } from "../../components/InviteMembers";
import { apiMessage, formatActivityTime, isAiActivity } from "../format";
import { ContributionCalendar } from "../common";
import { useActivity, useActivityCalendar, useCapabilities, useMembers } from "./data";
import { type WorkspacePage, sectionState } from "./page";
import { ICON_SIZE } from "../../components/ui/icon";

export function PeopleSection({ page }: { page: WorkspacePage }) {
  const { accessToken, workspace, workspaceId, session, canWrite, canManageMembers, canAssignAdmin, canManageWorkspace, isSubmitting, setIsSubmitting, setError, load } = page;
  const useS = sectionState(page.store, "people");
  const confirm = useConfirm();
  const [activityKind, setActivityKind] = useS<"user" | "ai">("activityKind", "user");
  const [memberEmail, setMemberEmail] = useS("memberEmail", "");
  const [memberRole, setMemberRole] = useS<WorkspaceRole>("memberRole", "member");
  const [inviteEmail, setInviteEmail] = useS("inviteEmail", "");
  const [capabilities] = useCapabilities(accessToken, workspaceId);
  const [members] = useMembers(accessToken, workspaceId, canManageMembers);
  const [activity] = useActivity(accessToken, workspaceId, canManageMembers);
  const [activityCalendar] = useActivityCalendar(accessToken, workspaceId, canManageMembers);
  const aiActivity = useMemo(() => activity.filter(isAiActivity), [activity]);
  const userActivity = useMemo(() => activity.filter((event) => !isAiActivity(event)), [activity]);
  const visibleActivity = activityKind === "ai" ? aiActivity : userActivity;
  async function saveMember(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      await upsertSharedWorkspaceMember(accessToken, workspace.workspace.id, {
        email: memberEmail.trim(),
        role: memberRole,
      });
      setMemberEmail("");
      setMemberRole("member");
      await load();
    } catch (requestError) {
      // Not in the organization yet (with or without an account): they join by invitation.
      if (requestError instanceof SharedApiError && requestError.status === 409) {
        setInviteEmail(memberEmail.trim());
        showToast("warning", `${memberEmail.trim()} is not in this organization yet. Create an invite link for them below.`);
      } else setError(apiMessage(requestError));
    } finally { setIsSubmitting(false); }
  }

  async function makeOwner(member: WorkspaceMember) {
    if (!(await confirm({ action: "Hand over", description: "You stay on as an administrator and cannot take it back yourself.", title: `Hand this workspace to ${member.user.display_name}?` }))) return;
    setIsSubmitting(true); setError(null);
    try {
      await transferSharedWorkspaceOwnership(accessToken, workspace.workspace.id, member.user.id);
      showToast("success", `${member.user.display_name} is now the owner.`);
      // Roles changed here and in the workspace list.
      window.location.reload();
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function removeMember(userId: string) {
    setIsSubmitting(true); setError(null);
    try { await removeSharedWorkspaceMember(accessToken, workspace.workspace.id, userId); await load(); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  return (
<>
            <div className="shared-members-section">
              <div className="shared-panel-heading"><div><Users size={ICON_SIZE.lg} /><h2>Workspace team</h2></div><span>{members.length}</span></div>
              <p className="shared-muted-copy">{canManageMembers ? "Add people and manage their access to this workspace." : "Your workspace role does not allow membership changes."}</p>
              <div className="shared-capabilities" aria-label="Your workspace capabilities" role="group">
                <strong>Your {capabilities?.role ?? workspace.role} access</strong>
                <span>Read evidence, retrieval, memory, exports, and activity.</span>
                <span>{canWrite ? "Create, edit, delete, and index shared evidence and memory." : "View-only: shared content cannot be changed."}</span>
                {canManageMembers ? <span>Manage workspace membership{canAssignAdmin ? " and assign administrator roles." : ", except administrator roles."}</span> : null}
                {canManageWorkspace ? <span>Rename or permanently delete this workspace.</span> : null}
              </div>
              <div className="shared-member-list">
                {members.map((member) => <article className={`shared-member-card role-${member.role}`} key={member.user.id}>
                  <UserAvatar className="shared-member-avatar" name={member.user.display_name} size={40} userId={member.user.id} />
                  <div><strong>{member.user.display_name}</strong><span>{member.user.email ?? "No email"}</span></div>
                  <span className="shared-member-role">{member.role}</span>
                  {workspace.role === "owner" && member.role !== "owner" && member.user.id !== session.user.id ? <Button disabled={isSubmitting} onClick={() => void makeOwner(member)} type="button" variant="secondary">Make owner</Button> : null}
                  {canManageMembers && member.role !== "owner" ? <Button className="shared-member-remove" disabled={isSubmitting} onClick={() => void removeMember(member.user.id)} type="button" variant="secondary">Remove</Button> : null}
                </article>)}
              </div>
              {canManageMembers ? <section className="shared-settings-group">
                <p className="shared-muted-copy">Add a registered RepoMemo user or update their role. Administrators cannot create, edit, or remove other administrators; owners retain the final authority.</p>
                <form className="shared-member-form" onSubmit={saveMember}><Input aria-label="Member email" onChange={(event) => setMemberEmail(event.target.value)} placeholder="person@example.com" required type="email" value={memberEmail} /><Dropdown aria-label="Member role" onValueChange={(value) => setMemberRole(value as WorkspaceRole)} options={[{ label: "Viewer", value: "viewer" }, { label: "Member", value: "member" }, ...(canAssignAdmin ? [{ label: "Admin", value: "admin" }] : [])]} value={memberRole} /><Button disabled={isSubmitting} type="submit" variant="secondary">Add / update</Button></form>
              </section> : null}
              {canManageMembers ? <InviteMembers accessToken={accessToken} initialEmail={inviteEmail} key={`invite-${workspace.workspace.id}`} roleOptions={[{ label: "Member", value: "member" }, { label: "Viewer", value: "viewer" }, ...(canAssignAdmin ? [{ label: "Admin", value: "admin" }] : [])]} targetId={workspace.workspace.id} targetLabel="workspace" targetType="workspace" /> : null}
            </div>
            <div className="shared-activity-section">
              <ContributionCalendar items={activityCalendar?.activity_by_day ?? []} title="Workspace activity" total={activityCalendar?.total_activity_count} />
              <div className="shared-panel-heading"><div><Timeline size={ICON_SIZE.lg} /><h2>Activity history</h2></div><span>{visibleActivity.length}</span></div>
              <SegmentedControl aria-label="Activity type" className="shared-activity-filter" onValueChange={setActivityKind} options={[{ count: userActivity.length, icon: <Users size={ICON_SIZE.sm} />, label: "User activity", value: "user" }, { count: aiActivity.length, icon: <Sparkles size={ICON_SIZE.sm} />, label: "AI activity", value: "ai" }]} value={activityKind} />
              {visibleActivity.length ? <div className="shared-activity-list">{visibleActivity.map((event) => <article key={event.id}><div><strong>{event.summary}</strong><span>{activityKind === "ai" ? `Requested by ${event.actor?.display_name ?? "Unknown"}` : (event.actor?.display_name ?? "System")} · {event.action.replace(/_/g, " ")}</span></div><time dateTime={event.created_at}>{formatActivityTime(event.created_at)}</time></article>)}</div> : <div className="shared-empty-state">{activityKind === "ai" ? <Sparkles size={25} /> : <Timeline size={25} />}<strong>{activityKind === "ai" ? "No AI activity yet" : "No recorded activity yet"}</strong><span>{activityKind === "ai" ? "Assistant questions and generated answers will appear here." : "New workspace changes will appear here."}</span></div>}
            </div>
</>
  );
}
