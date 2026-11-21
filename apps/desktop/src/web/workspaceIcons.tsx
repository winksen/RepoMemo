import { Plugs as Api, Lightning as Bolt, Book, Brain, Briefcase, Bank, Storefront as Store, Lightbulb as Bulb, ChartBar as Chart, Cloud, Code, Compass, Cpu, CreditCard, Database, DeviceMobile as Mobile, Flask, Heartbeat, Leaf, Lock, Chats as Messages, Palette, PuzzlePiece as Puzzle, RocketLaunch as Rocket, GraduationCap as School, HardDrives as Server, ShoppingCart as Cart, Stack as Layers, Toolbox as Tools, Truck, Users, Globe as World, type Icon as IconComponent, type IconWeight } from "@phosphor-icons/react";
import { Flag as FlagIcon, ShieldCheck as ShieldIcon, Target as TargetIcon, MapTrifold as MapIcon, CalendarBlank as CalendarIcon, Scales as ScalesIcon, Handshake as HandshakeIcon, Gear as GearIcon } from "@phosphor-icons/react";
import type { Workspace } from "../types";

/** The icons a workspace can take, by the key the server stores. */
export const WORKSPACE_ICONS: { key: string; label: string; icon: IconComponent }[] = [
  { key: "stack", label: "Layers", icon: Layers },
  { key: "rocket", label: "Rocket", icon: Rocket },
  { key: "code", label: "Code", icon: Code },
  { key: "api", label: "API", icon: Api },
  { key: "server", label: "Server", icon: Server },
  { key: "database", label: "Database", icon: Database },
  { key: "cloud", label: "Cloud", icon: Cloud },
  { key: "cpu", label: "Hardware", icon: Cpu },
  { key: "mobile", label: "Mobile", icon: Mobile },
  { key: "world", label: "Web", icon: World },
  { key: "lock", label: "Security", icon: Lock },
  { key: "bolt", label: "Energy", icon: Bolt },
  { key: "brain", label: "AI", icon: Brain },
  { key: "flask", label: "Research", icon: Flask },
  { key: "bulb", label: "Ideas", icon: Bulb },
  { key: "puzzle", label: "Integrations", icon: Puzzle },
  { key: "tools", label: "Tooling", icon: Tools },
  { key: "chart", label: "Analytics", icon: Chart },
  { key: "bank", label: "Finance", icon: Bank },
  { key: "card", label: "Payments", icon: CreditCard },
  { key: "cart", label: "Commerce", icon: Cart },
  { key: "store", label: "Retail", icon: Store },
  { key: "truck", label: "Logistics", icon: Truck },
  { key: "briefcase", label: "Business", icon: Briefcase },
  { key: "users", label: "People", icon: Users },
  { key: "messages", label: "Support", icon: Messages },
  { key: "book", label: "Documentation", icon: Book },
  { key: "school", label: "Training", icon: School },
  { key: "heartbeat", label: "Health", icon: Heartbeat },
  { key: "leaf", label: "Sustainability", icon: Leaf },
  { key: "palette", label: "Design", icon: Palette },
  { key: "compass", label: "Strategy", icon: Compass },
  { key: "flag", label: "Milestones", icon: FlagIcon },
  { key: "shield", label: "Compliance", icon: ShieldIcon },
  { key: "target", label: "Goals", icon: TargetIcon },
  { key: "map", label: "Planning", icon: MapIcon },
  { key: "calendar", label: "Schedule", icon: CalendarIcon },
  { key: "scales", label: "Legal", icon: ScalesIcon },
  { key: "handshake", label: "Partners", icon: HandshakeIcon },
  { key: "gear", label: "Operations", icon: GearIcon },
];

export const DEFAULT_WORKSPACE_ICON = WORKSPACE_ICONS[0].key;

/** The key of the icon a workspace shows; the default when none was chosen or the key is unknown. */
export function workspaceIconKey(workspace: Pick<Workspace, "settings">): string {
  const chosen = workspace.settings?.icon;
  return typeof chosen === "string" && WORKSPACE_ICONS.some((entry) => entry.key === chosen) ? chosen : DEFAULT_WORKSPACE_ICON;
}

/** The icon of a workspace, as chosen by its administrators. */
export function WorkspaceIcon({ size, weight = "fill", workspace }: { size: number | string; /** `fill` by default, where the icon stands by the workspace's name; the sidebar draws the others outlined. */ weight?: IconWeight; workspace: Pick<Workspace, "settings"> }) {
  const key = workspaceIconKey(workspace);
  const Icon = (WORKSPACE_ICONS.find((entry) => entry.key === key) ?? WORKSPACE_ICONS[0]).icon;
  return <Icon size={size} weight={weight} />;
}
