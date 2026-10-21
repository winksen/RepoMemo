import { type ChangeEvent, type FormEvent, useEffect, useRef, useState } from "react";
import { IconCamera as Camera, IconChecklist as Checklist, IconChevronRight as ChevronRight, IconKey as Key, IconLogout as Logout, IconPencil as Pencil, IconTrash as Trash, IconUpload as Upload, IconUserCircle as UserCircle } from "@tabler/icons-react";
import { type NotificationPreference, changeEmailAddress, getNotificationPreferences, resendEmailVerification, setNotificationPreference, changeSharedPassword, deleteSharedAvatar, fetchSharedAvatar, getSharedProfile, listSharedProfileTasks, signOutEverywhere, updateSharedProfile, uploadSharedAvatar } from "../lib/sharedApi";
import { type CollaborationTask, type Organization, type SharedSession, type SharedUser, type SharedWorkspace, type UserProfile } from "../types";
import { UserAvatar } from "../components/UserAvatar";
import { AVATAR_INPUT_TYPES, prepareAvatar } from "../lib/avatarImage";
import { Button } from "../components/ui/button";
import { PageHeader } from "../components/ui/page-header";
import { PanelHeading } from "../components/ui/panel-heading";
import { Dialog, DialogCancel } from "../components/ui/dialog";
import { Dropdown } from "../components/ui/dropdown";
import { EmptyState } from "../components/ui/empty-state";
import { Field } from "../components/ui/field";
import { Input } from "../components/ui/input";
import { showToast } from "../components/ui/toast";
import { ApiTokensPanel } from "../components/ApiTokensPanel";
import { accountKeys } from "../lib/queryClient";
import { useAsyncAction } from "../lib/useAsyncAction";
import { useFormState } from "../lib/useFormState";
import { useServerState } from "../lib/useServerState";
import { SharedLayout } from "../components/SharedLayout";
import { apiMessage, formatProfileTime, formatTaskDueDate } from "./format";
import { navigate } from "./router";
import { ContributionCalendar } from "./common";
import { OrganizationRail } from "./Organization";
import { ICON_SIZE } from "../components/ui/icon";

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
  const personal = useFormState({ displayName: profile?.user.display_name ?? session.user.display_name });
  const [preferences, setPreferences, preferencesQuery] = useServerState<NotificationPreference[]>({
    key: accountKeys.notificationPreferences(session.user.id), fetch: () => getNotificationPreferences(accessToken), initial: [], staleTime: 0,
  });
  const [newEmail, setNewEmail] = useState("");
  const [emailPassword, setEmailPassword] = useState("");
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const isLoading = profileQuery.isLoading || tasksQuery.isLoading;
  const { isBusy: isSubmitting, run } = useAsyncAction();
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
    await run(async () => {
      await uploadSharedAvatar(accessToken, session.user.id, await prepareAvatar(file));
      setHasAvatar(true);
    }, { success: "Profile picture updated." });
  }

  async function removeAvatar() {
    await run(async () => {
      await deleteSharedAvatar(accessToken, session.user.id);
      setHasAvatar(false);
    }, { success: "Profile picture removed." });
  }

  const loadError = profileQuery.error ?? tasksQuery.error;
  useEffect(() => { if (loadError) showToast("error", apiMessage(loadError)); }, [loadError]);

  async function saveProfile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await run(async () => {
      const user = await updateSharedProfile(accessToken, personal.values.displayName);
      onSessionUserUpdated(user);
      setProfile((current) => current ? { ...current, user, updated_at: new Date().toISOString() } : current);
    }, { success: "Profile name updated." });
  }

  async function chooseChannel(kind: string, channel: string) {
    try {
      setPreferences(await setNotificationPreference(accessToken, kind, channel));
      showToast("success", "Notification preference saved.");
    } catch (requestError) { showToast("error", apiMessage(requestError)); }
  }

  async function resendVerification() {
    await run(() => resendEmailVerification(accessToken), {
      success: (answer) => answer.verified ? "Your address is already confirmed." : answer.sent ? "A link was sent to your address." : "No link could be sent: mail is not set up on this server.",
    });
  }

  async function changeEmail(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await run(async () => {
      const user = await changeEmailAddress(accessToken, { email: newEmail, currentPassword: emailPassword });
      onSessionUserUpdated(user);
      setProfile((current) => current ? { ...current, user, email_verified: false } : current);
      setNewEmail(""); setEmailPassword("");
    }, { success: "Your address was changed. A link to confirm it was sent there." });
  }

  async function changePassword(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (newPassword !== confirmPassword) { showToast("error", "New password confirmation does not match."); return; }
    await run(async () => {
      await changeSharedPassword(accessToken, { currentPassword, newPassword });
      setCurrentPassword(""); setNewPassword(""); setConfirmPassword("");
    }, { success: "Password changed. You were signed out on every other device." });
  }

  async function endAllSessions() {
    await run(async () => {
      await signOutEverywhere(accessToken);
      setIsConfirmingSignOut(false);
      signOut();
    });
  }


  return <SharedLayout apiAvailable={apiAvailable} onNavigate={navigate} session={session} signOut={signOut} sidebar={<OrganizationRail organizations={organizations} workspaces={workspaces} />}>
    <section className="shared-page-content shared-profile-page" aria-busy={isLoading}>
      <PageHeader description="Manage your identity and review your activity across shared workspaces." title="Your profile" />
      <div className="shared-profile-layout">
        <section className="shared-profile-identity"><UserAvatar className="shared-profile-avatar" name={profile?.user.display_name ?? session.user.display_name} size={72} userId={session.user.id} /><div><h2>{profile?.user.display_name ?? session.user.display_name}</h2><p>{profile?.user.email ?? session.user.email ?? "No email address"}</p><div className="shared-profile-avatar-actions"><input accept={AVATAR_INPUT_TYPES.join(",")} aria-label="Choose a profile picture" hidden onChange={(event) => void changeAvatar(event)} ref={avatarInput} type="file" /><Button disabled={isSubmitting} onClick={() => avatarInput.current?.click()} type="button" variant="secondary"><Camera size={ICON_SIZE.sm} /> {hasAvatar ? "Change picture" : "Upload picture"}</Button>{hasAvatar ? <Button disabled={isSubmitting} onClick={() => void removeAvatar()} type="button" variant="secondary"><Trash size={ICON_SIZE.sm} /> Remove</Button> : null}</div><small className="shared-muted-copy">PNG, JPEG or WebP. It is cropped to a square and shrunk before upload.</small></div><dl><div><dt>Last connected</dt><dd>{formatProfileTime(profile?.last_connected_at)}</dd></div><div><dt>Member since</dt><dd>{formatProfileTime(profile?.created_at)}</dd></div><div><dt>Shared workspaces</dt><dd>{profile?.workspace_count ?? "—"}</dd></div><div><dt>Recorded actions</dt><dd>{profile?.recent_activity_count ?? "—"}</dd></div></dl></section>
        <ContributionCalendar items={profile?.activity_by_day ?? []} title="Your activity" total={profile?.recent_activity_count} />
        <section className="shared-profile-assigned"><PanelHeading description={<>{assignedTasks.length} active</>} icon={Checklist} title="Assigned to you" />{assignedTasks.length ? <div className="shared-profile-task-list">{assignedTasks.map((task) => <Button key={task.id} onClick={() => navigate(`/workspaces/${encodeURIComponent(task.workspace_id)}/tasks`)} type="button" variant="secondary"><span><strong>{task.title}</strong><small>{task.status.replace(/_/g, " ")} · {task.priority} priority{task.due_at ? ` · due ${formatTaskDueDate(task.due_at)}` : ""}</small></span><ChevronRight size={ICON_SIZE.md} /></Button>)}</div> : <EmptyState title="No active tasks are assigned to you" />}</section>
        <section className="shared-profile-panel"><PanelHeading icon={UserCircle} title="Personal details" /><form onSubmit={saveProfile}><Field label="Display name" required><Input autoComplete="name" onChange={(event) => personal.set("displayName", event.target.value)} value={personal.values.displayName} /></Field><Field label="Email"><Input disabled value={profile?.user.email ?? session.user.email ?? ""} /></Field><Button busy={isSubmitting} type="submit" variant="main"><Pencil size={ICON_SIZE.md} /> Save details</Button></form></section>
        <section className="shared-profile-panel"><PanelHeading description={profile?.email_verified === false ? "not confirmed" : "confirmed"} icon={UserCircle} title="Email address" />
          <p className="shared-muted-copy">{profile?.email_verified === false ? (profile.verification_required ? "Confirm your address with the link we sent: until then you cannot create organizations or workspaces." : "This address has not been confirmed.") : `Invitations and notices go to ${profile?.user.email ?? "your address"}.`}</p>
          {profile?.email_verified === false ? <Button busy={isSubmitting} onClick={() => void resendVerification()} variant="secondary">Send the link again</Button> : null}
          <form onSubmit={changeEmail}><Field label="New address" required><Input autoComplete="email" onChange={(event) => setNewEmail(event.target.value)} type="email" value={newEmail} /></Field><Field label="Current password" required><Input autoComplete="current-password" onChange={(event) => setEmailPassword(event.target.value)} type="password" value={emailPassword} /></Field><Button busy={isSubmitting} type="submit" variant="secondary">Change address</Button></form>
        </section>
        <section className="shared-profile-panel"><PanelHeading icon={Checklist} title="Notifications" />
          <p className="shared-muted-copy">Choose where each kind of notice reaches you. Password reset notices always reach you in the app and by mail.</p>
          {preferences.map((preference) => <Field key={preference.kind} label={preference.label}>{(control) => <Dropdown disabled={preferencesQuery.isLoading} id={control.id} onValueChange={(value) => void chooseChannel(preference.kind, value)} options={preference.kind === "daily_digest" ? [{ label: "Off", value: "none" }, { label: "By mail, once a day", value: "mail" }] : [{ label: "In the app and by mail", value: "both" }, { label: "In the app only", value: "app" }, { label: "By mail only", value: "mail" }, { label: "Nowhere", value: "none" }]} value={preference.channel} />}</Field>)}
        </section>
        <section className="shared-profile-panel"><PanelHeading icon={Key} title="Password" /><form onSubmit={changePassword}><Field label="Current password" required><Input autoComplete="current-password" minLength={12} onChange={(event) => setCurrentPassword(event.target.value)} type="password" value={currentPassword} /></Field><Field label="New password" required><Input autoComplete="new-password" minLength={12} onChange={(event) => setNewPassword(event.target.value)} placeholder="At least 12 characters" type="password" value={newPassword} /></Field><Field label="Confirm new password" required><Input autoComplete="new-password" minLength={12} onChange={(event) => setConfirmPassword(event.target.value)} type="password" value={confirmPassword} /></Field><Button busy={isSubmitting} type="submit" variant="secondary"><Key size={ICON_SIZE.md} /> Change password</Button></form></section>
        <section className="shared-profile-panel"><PanelHeading icon={Logout} title="Sessions" /><p className="shared-muted-copy">Signed in on a shared or lost device? End every session at once, this one included. Changing your password also ends your sessions on other devices.</p><Button disabled={isSubmitting} onClick={() => setIsConfirmingSignOut(true)} type="button" variant="secondary"><Logout size={ICON_SIZE.md} /> Sign out everywhere</Button></section>
        <ApiTokensPanel accessToken={accessToken} isAdministrator={Boolean(session.is_system_admin || session.is_app_admin)} />
        <Dialog description="Every device signed in to your account is signed out, including this one. You will need your password to sign in again." footer={<><DialogCancel onClick={() => setIsConfirmingSignOut(false)} /><Button busy={isSubmitting} onClick={() => void endAllSessions()} type="button" variant="main"><Logout size={ICON_SIZE.md} /> Sign out everywhere</Button></>} onClose={() => setIsConfirmingSignOut(false)} open={isConfirmingSignOut} title="Sign out of every session?" />
      </div>
    </section>
  </SharedLayout>;
}
