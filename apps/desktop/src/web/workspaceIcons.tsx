import {
  IconApi as Api,
  IconBolt as Bolt,
  IconBook2 as Book,
  IconBrain as Brain,
  IconBriefcase as Briefcase,
  IconBuildingBank as Bank,
  IconBuildingStore as Store,
  IconBulb as Bulb,
  IconChartBar as Chart,
  IconCloud as Cloud,
  IconCode as Code,
  IconCompass as Compass,
  IconCpu as Cpu,
  IconCreditCard as CreditCard,
  IconDatabase as Database,
  IconDeviceMobile as Mobile,
  IconFlask as Flask,
  IconHeartbeat as Heartbeat,
  IconLeaf as Leaf,
  IconLock as Lock,
  IconMessages as Messages,
  IconPalette as Palette,
  IconPuzzle as Puzzle,
  IconRocket as Rocket,
  IconSchool as School,
  IconServer as Server,
  IconShoppingCart as Cart,
  IconStack2 as Layers,
  IconTools as Tools,
  IconTruck as Truck,
  IconUsers as Users,
  IconWorld as World,
  type TablerIcon,
} from "@tabler/icons-react";
import type { Workspace } from "../types";

/** The icons a workspace can take, by the key the server stores. */
export const WORKSPACE_ICONS: { key: string; label: string; icon: TablerIcon }[] = [
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
];

export const DEFAULT_WORKSPACE_ICON = WORKSPACE_ICONS[0].key;

/** The key of the icon a workspace shows; the default when none was chosen or the key is unknown. */
export function workspaceIconKey(workspace: Pick<Workspace, "settings">): string {
  const chosen = workspace.settings?.icon;
  return typeof chosen === "string" && WORKSPACE_ICONS.some((entry) => entry.key === chosen) ? chosen : DEFAULT_WORKSPACE_ICON;
}

/** The icon of a workspace, as chosen by its administrators. */
export function WorkspaceIcon({ size, workspace }: { size: number | string; workspace: Pick<Workspace, "settings"> }) {
  const key = workspaceIconKey(workspace);
  const Icon = (WORKSPACE_ICONS.find((entry) => entry.key === key) ?? WORKSPACE_ICONS[0]).icon;
  return <Icon size={size} />;
}
