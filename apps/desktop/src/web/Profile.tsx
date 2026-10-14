import { type ChangeEvent, type FormEvent, useEffect, useRef, useState } from "react";
import { IconCamera as Camera, IconChecklist as Checklist, IconChevronRight as ChevronRight, IconKey as Key, IconLoader2 as Loader, IconLogout as Logout, IconPencil as Pencil, IconTrash as Trash, IconUpload as Upload, IconUserCircle as UserCircle } from "@tabler/icons-react";
import { changeSharedPassword, deleteSharedAvatar, fetchSharedAvatar, getSharedProfile, listSharedProfileTasks, signOutEverywhere, updateSharedProfile, uploadSharedAvatar } from "../lib/sharedApi";
import { type CollaborationTask, type Organization, type SharedSession, type SharedUser, type SharedWorkspace, type UserProfile } from "../types";
import { UserAvatar } from "../components/UserAvatar";
import { AVATAR_INPUT_TYPES, prepareAvatar } from "../lib/avatarImage";
import { Button } from "../components/ui/button";
import { Dialog, DialogCancel } from "../components/ui/dialog";
import { Input } from "../components/ui/input";
import { Toast } from "../components/ui/toast";
import { ApiTokensPanel } from "../components/ApiTokensPanel";
import { SharedLayout } from "../components/SharedLayout";
import { apiMessage, formatProfileTime, formatTaskDueDate } from "./format";
import { navigate } from "./router";
import { ContributionCalendar } from "./common";
import { OrganizationRail } from "./Organization";

export function SharedProfile({
  accessToken,
  apiAvailable,
  organizations,
  onSessionUserUpdated,
  session,
  signOut,
  workspaces,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  organizations: Organization[];
  onSessionUserUpdated: (user: SharedUser) => void;
  session: SharedSession;
  signOut: () => void;
  workspaces: SharedWorkspace[];
}) {
  const [profile, setProfile] = useState<UserProfile | null>(null);
  const [assignedTasks, setAssignedTasks] = useState<CollaborationTask[]>([]);
  const [displayName, setDisplayName] = useState(session.user.display_name);
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isConfirmingSignOut, setIsConfirmingSignOut] = useState(false);
  const [hasAvatar, setHasAvatar] = useState(false);
  const avatarInput = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let active = true;
    void fetchSharedAvatar(session.user.id).then((url) => { if (active) setHasAvatar(Boolean(url)); });
    return () => { active = false; };
  }, [session.user.id]);

  async function changeAvatar(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    setIsSubmitting(true); setError(null); setNotice(null);
    try {
      await uploadSharedAvatar(accessToken, session.user.id, await prepareAvatar(file));
      setHasAvatar(true);
      setNotice("Profile picture updated.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function removeAvatar() {
    setIsSubmitting(true); setError(null); setNotice(null);
    try {
      await deleteSharedAvatar(accessToken, session.user.id);
      setHasAvatar(false);
      setNotice("Profile picture removed.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function load() {
    setIsLoading(true); setError(null);
    try {
      const [nextProfile, nextAssignedTasks] = await Promise.all([
        getSharedProfile(accessToken),
        listSharedProfileTasks(accessToken),
      ]);
      setProfile(nextProfile);
      setAssignedTasks(nextAssignedTasks);
      setDisplayName(nextProfile.user.display_name);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsLoading(false); }
  }

  useEffect(() => { void load(); }, [accessToken]);

  async function saveProfile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null); setNotice(null);
    try {
      const user = await updateSharedProfile(accessToken, displayName);
      onSessionUserUpdated(user);
      setProfile((current) => current ? { ...current, user, updated_at: new Date().toISOString() } : current);
      setNotice("Profile name updated.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function changePassword(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (newPassword !== confirmPassword) { setError("New password confirmation does not match."); return; }
    setIsSubmitting(true); setError(null); setNotice(null);
    try {
      await changeSharedPassword(accessToken, { currentPassword, newPassword });
      setCurrentPassword(""); setNewPassword(""); setConfirmPassword("");
      setNotice("Password changed. You were signed out on every other device.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function endAllSessions() {
    setIsSubmitting(true); setError(null); setNotice(null);
    try {
      await signOutEverywhere(accessToken);
      setIsConfirmingSignOut(false);
      signOut();
    } catch (requestError) { setError(apiMessage(requestError)); setIsSubmitting(false); }
  }


  return <SharedLayout apiAvailable={apiAvailable} onNavigate={navigate} session={session} signOut={signOut} sidebar={<OrganizationRail organizations={organizations} workspaces={workspaces} />}>
    <section className="shared-page-content shared-profile-page" aria-busy={isLoading}>
      <div className="shared-detail-heading"><div><h1>Your profile</h1><p>Manage your identity and review your activity across shared workspaces.</p></div></div>
      <Toast kind="error" message={error} />
      <Toast kind="success" message={notice} />
      <div className="shared-profile-layout">
        <section className="shared-profile-identity"><UserAvatar className="shared-profile-avatar" name={profile?.user.display_name ?? session.user.display_name} size={72} userId={session.user.id} /><div><h2>{profile?.user.display_name ?? session.user.display_name}</h2><p>{profile?.user.email ?? session.user.email ?? "No email address"}</p><div className="shared-profile-avatar-actions"><input accept={AVATAR_INPUT_TYPES.join(",")} aria-label="Choose a profile picture" hidden onChange={(event) => void changeAvatar(event)} ref={avatarInput} type="file" /><Button disabled={isSubmitting} onClick={() => avatarInput.current?.click()} type="button" variant="secondary"><Camera size={15} /> {hasAvatar ? "Change picture" : "Upload picture"}</Button>{hasAvatar ? <Button disabled={isSubmitting} onClick={() => void removeAvatar()} type="button" variant="secondary"><Trash size={15} /> Remove</Button> : null}</div><small className="shared-muted-copy">PNG, JPEG or WebP. It is cropped to a square and shrunk before upload.</small></div><dl><div><dt>Last connected</dt><dd>{formatProfileTime(profile?.last_connected_at)}</dd></div><div><dt>Member since</dt><dd>{formatProfileTime(profile?.created_at)}</dd></div><div><dt>Shared workspaces</dt><dd>{profile?.workspace_count ?? "—"}</dd></div><div><dt>Recorded actions</dt><dd>{profile?.recent_activity_count ?? "—"}</dd></div></dl></section>
        <ContributionCalendar items={profile?.activity_by_day ?? []} title="Your activity" total={profile?.recent_activity_count} />
        <section className="shared-profile-assigned"><div className="shared-panel-heading"><div><Checklist size={18} /><h2>Assigned to you</h2></div><span>{assignedTasks.length} active</span></div>{assignedTasks.length ? <div className="shared-profile-task-list">{assignedTasks.map((task) => <Button key={task.id} onClick={() => navigate(`/workspaces/${encodeURIComponent(task.workspace_id)}/tasks`)} type="button" variant="secondary"><span><strong>{task.title}</strong><small>{task.status.replace(/_/g, " ")} · {task.priority} priority{task.due_at ? ` · due ${formatTaskDueDate(task.due_at)}` : ""}</small></span><ChevronRight size={16} /></Button>)}</div> : <p className="shared-muted-copy">No active tasks are assigned to you.</p>}</section>
        <section className="shared-profile-panel"><div className="shared-panel-heading"><div><UserCircle size={18} /><h2>Personal details</h2></div></div><form onSubmit={saveProfile}><label>Display name<Input autoComplete="name" onChange={(event) => setDisplayName(event.target.value)} required value={displayName} /></label><label>Email<Input disabled value={profile?.user.email ?? session.user.email ?? ""} /></label><Button disabled={isSubmitting} type="submit" variant="main">{isSubmitting ? <Loader className="spin" size={16} /> : <Pencil size={16} />} Save details</Button></form></section>
        <section className="shared-profile-panel"><div className="shared-panel-heading"><div><Key size={18} /><h2>Password</h2></div></div><form onSubmit={changePassword}><label>Current password<Input autoComplete="current-password" minLength={12} onChange={(event) => setCurrentPassword(event.target.value)} required type="password" value={currentPassword} /></label><label>New password<Input autoComplete="new-password" minLength={12} onChange={(event) => setNewPassword(event.target.value)} placeholder="At least 12 characters" required type="password" value={newPassword} /></label><label>Confirm new password<Input autoComplete="new-password" minLength={12} onChange={(event) => setConfirmPassword(event.target.value)} required type="password" value={confirmPassword} /></label><Button disabled={isSubmitting} type="submit" variant="secondary">{isSubmitting ? <Loader className="spin" size={16} /> : <Key size={16} />} Change password</Button></form></section>
        <section className="shared-profile-panel"><div className="shared-panel-heading"><div><Logout size={18} /><h2>Sessions</h2></div></div><p className="shared-muted-copy">Signed in on a shared or lost device? End every session at once, this one included. Changing your password also ends your sessions on other devices.</p><Button disabled={isSubmitting} onClick={() => setIsConfirmingSignOut(true)} type="button" variant="secondary"><Logout size={16} /> Sign out everywhere</Button></section>
        <ApiTokensPanel accessToken={accessToken} isAdministrator={Boolean(session.is_system_admin || session.is_app_admin)} />
        <Dialog description="Every device signed in to your account is signed out, including this one. You will need your password to sign in again." footer={<><DialogCancel onClick={() => setIsConfirmingSignOut(false)} /><Button disabled={isSubmitting} onClick={() => void endAllSessions()} type="button" variant="main">{isSubmitting ? <Loader className="spin" size={16} /> : <Logout size={16} />} Sign out everywhere</Button></>} onClose={() => setIsConfirmingSignOut(false)} open={isConfirmingSignOut} title="Sign out of every session?" />
      </div>
    </section>
  </SharedLayout>;
}
