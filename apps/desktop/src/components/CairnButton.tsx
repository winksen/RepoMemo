import * as MenuPrimitive from "@radix-ui/react-dropdown-menu";
import { CaretDown, ChatCircleText, Lightning } from "@phosphor-icons/react";
import { openQuickAsk } from "../lib/cairnBridge";
import { ASSISTANT_NAME } from "../lib/assistantName";
import { rememberCairnWorkspace } from "../web/cairn";
import { parseRoute } from "../web/router";
import { CairnIcon } from "./CairnIcon";
import { Button } from "./ui/button";
import { ICON_SIZE } from "./ui/icon";

/**
 * CAIRN in the top bar: a button with its name that opens its page, and a menu beside it for the quick ways in (a quick question
 * in a dialog, a new chat). From a workspace page, CAIRN starts in that workspace.
 */
export function CairnButton({ onNavigate }: { onNavigate: (to: string) => void }) {
  const onCairn = window.location.pathname === "/cairn" || window.location.pathname.startsWith("/cairn/");
  function remember() {
    const route = parseRoute(window.location.pathname);
    if (route.name === "workspace" || route.name === "artifact" || route.name === "memory-card") rememberCairnWorkspace(route.workspaceId);
  }
  return (
    <div className="shared-cairn-split">
      <Button aria-current={onCairn ? "page" : undefined} className="shared-cairn-main" onClick={() => { remember(); onNavigate("/cairn"); }} type="button" variant="secondary">
        <CairnIcon size={ICON_SIZE.lg} weight={onCairn ? "fill" : "regular"} /><span>{ASSISTANT_NAME}</span>
      </Button>
      <MenuPrimitive.Root>
        <MenuPrimitive.Trigger asChild>
          <Button aria-label={`Quick ways to use ${ASSISTANT_NAME}`} className="shared-cairn-caret" type="button" variant="secondary"><CaretDown size={ICON_SIZE.sm} /></Button>
        </MenuPrimitive.Trigger>
        <MenuPrimitive.Portal>
          <MenuPrimitive.Content align="end" aria-label={`Quick ways to use ${ASSISTANT_NAME}`} className="rm-action-menu" sideOffset={6}>
            <MenuPrimitive.Item className="rm-action-item" onSelect={() => { remember(); openQuickAsk(); }}><Lightning size={ICON_SIZE.md} /><span>Quick question</span></MenuPrimitive.Item>
            <MenuPrimitive.Item className="rm-action-item" onSelect={() => { remember(); onNavigate("/cairn"); }}><ChatCircleText size={ICON_SIZE.md} /><span>New chat</span></MenuPrimitive.Item>
          </MenuPrimitive.Content>
        </MenuPrimitive.Portal>
      </MenuPrimitive.Root>
    </div>
  );
}
