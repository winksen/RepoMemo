import * as MenuPrimitive from "@radix-ui/react-dropdown-menu";
import { CaretRight, Gear, SignOut, UserCircle } from "@phosphor-icons/react";
import { lastSignIn } from "../lib/signedIn";
import type { SharedSession } from "../types";
import { ICON_SIZE } from "./ui/icon";
import { UserAvatar } from "./UserAvatar";

/**
 * The person's menu at the foot of the sidebar: who is signed in, and a dropdown that opens to the right with their profile,
 * their settings and sign out (more entries will join them). It replaces the session note that used to sit here.
 */
export function UserMenu({ onNavigate, session, signOut }: { onNavigate: (to: string) => void; session: SharedSession; signOut: () => void }) {
  const name = session.user.display_name;
  const since = lastSignIn();
  return (
    <div className="shared-user-foot">
      <MenuPrimitive.Root>
        <MenuPrimitive.Trigger asChild>
          <button aria-label={`Account menu for ${name}`} className="shared-user-trigger" type="button">
            <UserAvatar name={name} size={36} userId={session.user.id} />
            <span className="shared-user-trigger-text"><strong>{name}</strong><small>{session.user.email ?? "Signed in"}</small></span>
            <CaretRight aria-hidden="true" size={ICON_SIZE.sm} />
          </button>
        </MenuPrimitive.Trigger>
        <MenuPrimitive.Portal>
          <MenuPrimitive.Content align="end" aria-label="Account" className="rm-action-menu shared-user-dropdown" side="right" sideOffset={10}>
            <MenuPrimitive.Label className="shared-user-dropdown-who">
              <strong>{name}</strong>
              <span>{since ? `Signed in ${since.toLocaleString([], { dateStyle: "medium", timeStyle: "short" })}` : "Session connected"}</span>
            </MenuPrimitive.Label>
            <MenuPrimitive.Separator className="shared-user-dropdown-line" />
            <MenuPrimitive.Item className="rm-action-item" onSelect={() => onNavigate("/profile")}><UserCircle size={ICON_SIZE.md} /><span>Profile</span></MenuPrimitive.Item>
            <MenuPrimitive.Item className="rm-action-item" onSelect={() => onNavigate("/profile#settings")}><Gear size={ICON_SIZE.md} /><span>Settings</span></MenuPrimitive.Item>
            <MenuPrimitive.Separator className="shared-user-dropdown-line" />
            <MenuPrimitive.Item className="rm-action-item shared-user-dropdown-out" onSelect={signOut}><SignOut size={ICON_SIZE.md} /><span>Log out</span></MenuPrimitive.Item>
          </MenuPrimitive.Content>
        </MenuPrimitive.Portal>
      </MenuPrimitive.Root>
    </div>
  );
}
