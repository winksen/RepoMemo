import * as MenuPrimitive from "@radix-ui/react-dropdown-menu";
import { IconBuildingCommunity as Building, IconPlus as Plus, IconStack2 as Layers } from "@tabler/icons-react";
import { navigate } from "../web/router";
import { IconButton } from "./ui/icon-button";
import { ICON_SIZE } from "./ui/icon";

/** The plus button of the navbar: start a new organization or a new workspace from any page. */
export function CreateMenu({ canCreateOrganization }: { canCreateOrganization: boolean }) {
  function newWorkspace() {
    // Keep the organization being viewed, when the address names one; the dashboard falls back to the first.
    const organization = new URLSearchParams(window.location.search).get("organization");
    navigate(`/workspaces?${organization ? `organization=${encodeURIComponent(organization)}&` : ""}createWorkspace=1`);
  }
  return (
    <MenuPrimitive.Root>
      <MenuPrimitive.Trigger asChild>
        <IconButton className="shared-create-trigger" label="Create new" size="sm"><Plus size={ICON_SIZE.md} /></IconButton>
      </MenuPrimitive.Trigger>
      <MenuPrimitive.Portal>
        <MenuPrimitive.Content align="end" aria-label="Create new" className="rm-action-menu" sideOffset={6}>
          <MenuPrimitive.Item className="rm-action-item" onSelect={newWorkspace}><Layers size={ICON_SIZE.md} /><span>New workspace</span></MenuPrimitive.Item>
          {canCreateOrganization ? <MenuPrimitive.Item className="rm-action-item" onSelect={() => navigate("/workspaces?createOrganization=1")}><Building size={ICON_SIZE.md} /><span>New organization</span></MenuPrimitive.Item> : null}
        </MenuPrimitive.Content>
      </MenuPrimitive.Portal>
    </MenuPrimitive.Root>
  );
}
