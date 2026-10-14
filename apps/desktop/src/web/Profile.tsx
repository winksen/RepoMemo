import { type ChangeEvent, type FormEvent, useEffect, useRef, useState } from "react";
import { IconCamera as Camera, IconChecklist as Checklist, IconChevronRight as ChevronRight, IconKey as Key, IconLoader2 as Loader, IconLogout as Logout, IconPencil as Pencil, IconTrash as Trash, IconUpload as Upload, IconUserCircle as UserCircle } from "@tabler/icons-react";
import { type NotificationPreference, changeEmailAddress, getNotificationPreferences, resendEmailVerification, setNotificationPreference, changeSharedPassword, deleteSharedAvatar, fetchSharedAvatar, getSharedProfile, listSharedProfileTasks, signOutEverywhere, updateSharedProfile, uploadSharedAvatar } from "../lib/sharedApi";
import { type CollaborationTask, type Organization, type SharedSession, type SharedUser, type SharedWorkspace, type UserProfile } from "../types";
import { UserAvatar } from "../components/UserAvatar";
import { AVATAR_INPUT_TYPES, prepareAvatar } from "../lib/avatarImage";
import { Button } from "../components/ui/button";
import { Dialog, DialogCancel } from "../components/ui/dialog";
import { Dropdown } from "../components/ui/dropdown";
import { Input } from "../components/ui/input";
import { Toast } from "../components/ui/toast";
import { ApiTokensPanel } from "../components/ApiTokensPanel";
import { accountKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
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
  // Profile and assigned tasks come from the query cache: shown at once on a
  // later visit and refetched when the page opens.
  const [profile, setProfile, profileQuery] = useServerState<UserProfile | null>({
    key: accountKeys.profile(session.user.id), fetch: () => getSharedProfile(accessToken), initial: null, staleTime: 0,
  });
  const [assignedTasks, , tasksQuery] = useServerState<CollaborationTask[]>({
    key: accountKeys.profileTasks(session.user.id), fetch: () => listSharedProfileTasks(accessToken), initial: [], staleTime: 0,
  });
  const [displayName, setDisplayName] = useState(session.user.display_name);
  const [preferences, setPreferences, preferencesQuery] = useServerState<NotificationPreference[]>({
    key: accountKeys.notificationPreferences(session.user.id), fetch: () => getNotificationPreferences(accessToken), initial: [], staleTime: 0,
  });
  const [newEmail, setNewEmail] = useState("");
  const [emailPassword, setEmailPassword] = useState("");
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const isLoading = profileQuery.isLoading || tasksQuery.isLoading;
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

  const loadError = profileQuery.error ?? tasksQuery.error;
  useEffect(() => { if (loadError) setError(apiMessage(loadError)); }, [loadError]);
  useEffect(() => { if (profile) setDisplayName(profile.user.display_name); }, [profile?.user.display_name]);

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

  async function chooseChannel(kind: string, channel: string) {
    try {
      setPreferences(await setNotificationPreference(accessToken, kind, channel));
    } catch (requestError) { setError(apiMessage(requestError)); }
  }

  async function resendVerification() {
    setIsSubmitting(true); setError(null);
    try {
      const answer = await resendEmailVerification(accessToken);
      setNotice(answer.verified ? "Your address is already confirmed." : answer.sent ? "A link was sent to your address." : "No link could be sent: mail is not set up on this server.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function changeEmail(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null); setNotice(null);
    try {
      const user = await changeEmailAddress(accessToken, { email: newEmail, currentPassword: emailPassword });
      onSessionUserUpdated(user);
      setProfile((current) => current ? { ...current, user, email_verified: false } : current);
      setNewEmail(""); setEmailPassword("");
      setNotice("Your address was changed. A link to confirm it was sent there.");
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
        <section className="shared-profile-panel"><div className="shared-panel-heading"><div><UserCircle size={18} /><h2>Email address</h2></div><span>{profile?.email_verified === false ? "not confirmed" : "confirmed"}</span></div>
          <p className="shared-muted-copy">{profile?.email_verified === false ? (profile.verification_required ? "Confirm your address with the link we sent: until then you cannot create organizations or workspaces." : "This address has not been confirmed.") : `Invitations and notices go to ${profile?.user.email ?? "your address"}.`}</p>
          {profile?.email_verified === false ? <Button disabled={isSubmitting} onClick={() => void resendVerification()} type="button" variant="secondary">Send the link again</Button> : null}
          <form onSubmit={changeEmail}><label>New address<Input autoComplete="email" onChange={(event) => setNewEmail(event.target.value)} required type="email" value={newEmail} /></label><label>Current password<Input autoComplete="current-password" onChange={(event) => setEmailPassword(event.target.value)} required type="password" value={emailPassword} /></label><Button disabled={isSubmitting} type="submit" variant="secondary">Change address</Button></form>
        </section>
        <section className="shared-profile-panel"><div className="shared-panel-heading"><div><Checklist size={18} /><h2>Notifications</h2></div></div>
          <p className="shared-muted-copy">Choose where each kind of notice reaches you. Password reset notices always reach you in the app and by mail.</p>
          {preferences.map((preference) => <label key={preference.kind}>{preference.label}<Dropdown aria-label={preference.label} disabled={preferencesQuery.isLoading} onValueChange={(value) => void chooseChannel(preference.kind, value)} options={[{ label: "In the app and by mail", value: "both" }, { label: "In the app only", value: "app" }, { label: "By mail only", value: "mail" }, { label: "Nowhere", value: "none" }]} value={preference.channel} /></label>)}
        </section>
        <section className="shared-profile-panel"><div className="shared-panel-heading"><div><Key size={18} /><h2>Password</h2></div></div><form onSubmit={changePassword}><label>Current password<Input autoComplete="current-password" minLength={12} onChange={(event) => setCurrentPassword(event.target.value)} required type="password" value={currentPassword} /></label><label>New password<Input autoComplete="new-password" minLength={12} onChange={(event) => setNewPassword(event.target.value)} placeholder="At least 12 characters" required type="password" value={newPassword} /></label><label>Confirm new password<Input autoComplete="new-password" minLength={12} onChange={(event) => setConfirmPassword(event.target.value)} required type="password" value={confirmPassword} /></label><Button disabled={isSubmitting} type="submit" variant="secondary">{isSubmitting ? <Loader className="spin" size={16} /> : <Key size={16} />} Change password</Button></form></section>
        <section className="shared-profile-panel"><div className="shared-panel-heading"><div><Logout size={18} /><h2>Sessions</h2></div></div><p className="shared-muted-copy">Signed in on a shared or lost device? End every session at once, this one included. Changing your password also ends your sessions on other devices.</p><Button disabled={isSubmitting} onClick={() => setIsConfirmingSignOut(true)} type="button" variant="secondary"><Logout size={16} /> Sign out everywhere</Button></section>
        <ApiTokensPanel accessToken={accessToken} isAdministrator={Boolean(session.is_system_admin || session.is_app_admin)} />
        <Dialog description="Every device signed in to your account is signed out, including this one. You will need your password to sign in again." footer={<><DialogCancel onClick={() => setIsConfirmingSignOut(false)} /><Button disabled={isSubmitting} onClick={() => void endAllSessions()} type="button" variant="main">{isSubmitting ? <Loader className="spin" size={16} /> : <Logout size={16} />} Sign out everywhere</Button></>} onClose={() => setIsConfirmingSignOut(false)} open={isConfirmingSignOut} title="Sign out of every session?" />
      </div>
    </section>
  </SharedLayout>;
}
