import { Bank, Buildings, BuildingApartment, BuildingOffice, Factory, FirstAidKit, Flask, GraduationCap, House, HouseLine, Storefront, UsersThree, Warehouse, type Icon as IconComponent, type IconWeight } from "@phosphor-icons/react";
import type { Organization } from "../types";

/** The icons an organization can take, by the key the server stores: places a team works from. */
export const ORGANIZATION_ICONS: { key: string; label: string; icon: IconComponent }[] = [
  { key: "building", label: "Company", icon: Buildings },
  { key: "office", label: "Office", icon: BuildingOffice },
  { key: "apartment", label: "Apartment", icon: BuildingApartment },
  { key: "house", label: "House", icon: House },
  { key: "studio", label: "Studio", icon: HouseLine },
  { key: "factory", label: "Factory", icon: Factory },
  { key: "bank", label: "Bank", icon: Bank },
  { key: "shop", label: "Shop", icon: Storefront },
  { key: "school", label: "School", icon: GraduationCap },
  { key: "clinic", label: "Clinic", icon: FirstAidKit },
  { key: "lab", label: "Lab", icon: Flask },
  { key: "warehouse", label: "Warehouse", icon: Warehouse },
  { key: "community", label: "Community", icon: UsersThree },
];

export const DEFAULT_ORGANIZATION_ICON = ORGANIZATION_ICONS[0].key;

/** The key of the icon an organization shows; the default when none was chosen or the key is unknown. */
export function organizationIconKey(organization: Pick<Organization, "icon"> | { icon?: string | null }): string {
  const chosen = organization.icon;
  return typeof chosen === "string" && ORGANIZATION_ICONS.some((entry) => entry.key === chosen) ? chosen : DEFAULT_ORGANIZATION_ICON;
}

/** The icon of an organization. */
export function OrganizationIcon({ organization, size, weight = "regular" }: { organization: Pick<Organization, "icon"> | { icon?: string | null }; size: number | string; weight?: IconWeight }) {
  const Icon = (ORGANIZATION_ICONS.find((entry) => entry.key === organizationIconKey(organization)) ?? ORGANIZATION_ICONS[0]).icon;
  return <Icon size={size} weight={weight} />;
}
