import { createContext, type ReactNode, useContext, useEffect, useState } from "react";
import { type Organization } from "../types";
import { type OrganizationNavigation } from "./constants";
import { currentLocation, navigate, organizationPath, parseRoute } from "./router";
import { SharedWebApp } from "../SharedWebApp";

export const OrganizationNavigationContext = createContext<OrganizationNavigation | null>(null);

export function OrganizationNavigationProvider({ children }: { children: ReactNode }) {
  const [location, setLocation] = useState(currentLocation);
  const queryIndex = location.indexOf("?");
  const parameters = new URLSearchParams(queryIndex === -1 ? "" : location.slice(queryIndex + 1));
  const route = parseRoute(queryIndex === -1 ? location : location.slice(0, queryIndex));
  const activeOrganizationId = route.name === "organization" ? route.organizationId : null;
  const isCreatingOrganization = route.name === "new-organization";

  useEffect(() => {
    const syncLocation = () => setLocation(currentLocation());
    window.addEventListener("popstate", syncLocation);
    return () => window.removeEventListener("popstate", syncLocation);
  }, []);

  const value: OrganizationNavigation = {
    activeOrganizationId,
    isCreatingOrganization,
    selectOrganization: (organizationId) => navigate(organizationPath(organizationId)),
    startCreatingOrganization: () => navigate("/orgs/new"),
    cancelOrganizationCreation: () => navigate(activeOrganizationId ? organizationPath(activeOrganizationId) : "/orgs"),
  };

  return <OrganizationNavigationContext.Provider value={value}>{children}</OrganizationNavigationContext.Provider>;
}

export function useOrganizationNavigation() {
  const navigation = useContext(OrganizationNavigationContext);
  if (!navigation) throw new Error("Organization navigation is only available inside SharedWebApp.");
  return navigation;
}
