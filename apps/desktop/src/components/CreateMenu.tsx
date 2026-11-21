import * as MenuPrimitive from "@radix-ui/react-dropdown-menu";
import { Buildings as Building, Plus, Stack as Layers } from "@phosphor-icons/react";
import { openNewWorkspace } from "../lib/newWorkspaceBridge";
import { navigate, parseRoute } from "../web/router";
import { IconButton } from "./ui/icon-button";
import { ICON_SIZE } from "./ui/icon";

/** The plus button of the navbar: start a new organization or a new workspace from any page. */
export function CreateMenu({ canCreateOrganization, defaultOrganizationId }: { canCreateOrganization: boolean; defaultOrganizationId?: string }) {
  function newWorkspace() {
    // The organization being viewed, else the one the shell proposes; the dialog lets the person pick another.
    const route = parseRoute(window.location.pathname);
    openNewWorkspace(route.name === "organization" ? route.organizationId : defaultOrganizationId);
  }
  return (
    <MenuPrimitive.Root>
      <MenuPrimitive.Trigger asChild>
        <IconButton className="shared-create-trigger" label="Create new" size="sm"><Plus size={ICON_SIZE.md} /></IconButton>
      </MenuPrimitive.Trigger>
      <MenuPrimitive.Portal>
        <MenuPrimitive.Content align="end" aria-label="Create new" className="rm-action-menu" sideOffset={6}>
          <MenuPrimitive.Item className="rm-action-item" onSelect={newWorkspace}><Layers size={ICON_SIZE.md} /><span>New workspace</span></MenuPrimitive.Item>
          {canCreateOrganization ? <MenuPrimitive.Item className="rm-action-item" onSelect={() => navigate("/orgs/new")}><Building size={ICON_SIZE.md} /><span>New organization</span></MenuPrimitive.Item> : null}
        </MenuPrimitive.Content>
      </MenuPrimitive.Portal>
    </MenuPrimitive.Root>
  );
}
