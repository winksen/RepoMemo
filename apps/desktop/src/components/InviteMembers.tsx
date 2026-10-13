import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { IconLink as LinkIcon, IconMailForward as MailForward, IconTrash as Trash } from "@tabler/icons-react";
import { createSharedInvitation, listSharedInvitations, revokeSharedInvitation } from "../lib/sharedApi";
import type { InvitationTarget } from "../lib/sharedApi";
import type { Invitation } from "../types";
import { appLink, SecretDialog } from "./SecretDialog";
import { Button } from "./ui/button";
import { Dropdown } from "./ui/dropdown";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}

function formatDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleDateString(undefined, { dateStyle: "medium" });
}

/**
 * Invites someone who may not have an account yet: the administrator creates
 * a link for one email address and sends it themselves (chat, email…). The
 * person opens it, creates an account or signs in, and joins with the chosen
 * role. Links last a week and work once; pending ones can be withdrawn.
 */
export function InviteMembers({
  accessToken,
  initialEmail,
  onInvited,
  roleOptions,
  targetId,
  targetLabel,
  targetType,
}: {
  accessToken: string;
  /** An address to start with, such as one that has no account yet. */
  initialEmail?: string;
  onInvited?: () => void;
  roleOptions: Array<{ label: string; value: string }>;
  targetId: string;
  /** "workspace" or "organization", for the copy. */
  targetLabel: string;
  targetType: InvitationTarget;
}) {
  const [invitations, setInvitations] = useState<Invitation[] | null>(null);
  const [email, setEmail] = useState(initialEmail ?? "");
  const [role, setRole] = useState(roleOptions[0]?.value ?? "member");
  const [link, setLink] = useState<{ email: string; url: string } | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  async function load() {
    try { setInvitations(await listSharedInvitations(accessToken, targetType, targetId)); }
    catch (error) { showToast("error", errorMessage(error, "Pending invitations could not be loaded.")); setInvitations([]); }
  }

  useEffect(() => { void load(); }, [accessToken, targetId, targetType]);
  useEffect(() => { if (initialEmail) setEmail(initialEmail); }, [initialEmail]);

  async function invite(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsBusy(true);
    try {
      const created = await createSharedInvitation(accessToken, targetType, targetId, { email: email.trim(), role });
      setLink({ email: created.invitation.email, url: appLink(created.path) });
      setEmail("");
      onInvited?.();
      await load();
    } catch (error) { showToast("error", errorMessage(error, "The invitation could not be created.")); }
    finally { setIsBusy(false); }
  }

  async function withdraw(invitation: Invitation) {
    setIsBusy(true);
    try {
      await revokeSharedInvitation(accessToken, invitation.id);
      setInvitations((current) => current?.filter((entry) => entry.id !== invitation.id) ?? current);
      showToast("success", `The invitation of ${invitation.email} was withdrawn.`);
    } catch (error) { showToast("error", errorMessage(error, "The invitation could not be withdrawn.")); }
    finally { setIsBusy(false); }
  }

  return <section className="shared-settings-group rm-invite" aria-label={`Invite to this ${targetLabel}`}>
    <div className="shared-panel-heading"><div><MailForward size={18} /><h2>Invite by link</h2></div><span>{invitations?.length ?? 0} pending</span></div>
    <p className="shared-muted-copy">For people without an account, or to send an access link. You get a link to pass on yourself; it works once, for this email address only, and expires after 7 days. They can create their account from it even when registration is closed.</p>
    <form className="rm-invite-form" onSubmit={invite}>
      <Input aria-label="Email to invite" autoComplete="off" onChange={(event) => setEmail(event.target.value)} placeholder="person@example.com" required type="email" value={email} />
      <Dropdown aria-label="Role for the invited person" onValueChange={setRole} options={roleOptions} value={role} />
      <Button disabled={isBusy || !email.trim()} type="submit" variant="secondary"><LinkIcon size={16} /> Create invite link</Button>
    </form>
    {invitations?.length ? <ul className="rm-invite-list">{invitations.map((invitation) => <li key={invitation.id}>
      <span className="rm-invite-text"><strong>{invitation.email}</strong><small>{invitation.role} · expires {formatDate(invitation.expires_at)}{invitation.invited_by ? ` · by ${invitation.invited_by.display_name}` : ""}</small></span>
      <Button aria-label={`Withdraw the invitation of ${invitation.email}`} disabled={isBusy} onClick={() => void withdraw(invitation)} type="button" variant="secondary"><Trash size={15} /> Withdraw</Button>
    </li>)}</ul> : null}
    {link ? <SecretDialog description={`Send this link to ${link.email}. Opening it lets them create an account (or sign in) and join this ${targetLabel}.`} onClose={() => setLink(null)} title="Invitation link" value={link.url} /> : null}
  </section>;
}
