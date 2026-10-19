import type { ReactNode } from "react";
import {
  IconBuilding as Building,
  IconFileText as FileText,
  IconFlag as Flag,
  IconKey as Key,
  IconServer as Server,
  IconShieldLock as ShieldLock,
  IconUserShield as UserShield,
} from "@tabler/icons-react";

export type StepKey = "welcome" | "admin" | "checks" | "access" | "logging" | "organization" | "finish";

export const STEPS: { key: StepKey; label: string; icon: ReactNode }[] = [
  { key: "welcome", label: "Welcome", icon: <Key size={22} /> },
  { key: "admin", label: "System administrator", icon: <UserShield size={22} /> },
  { key: "checks", label: "Server check", icon: <Server size={22} /> },
  { key: "access", label: "Access and security", icon: <ShieldLock size={22} /> },
  { key: "logging", label: "Logging", icon: <FileText size={22} /> },
  { key: "organization", label: "First workspace", icon: <Building size={22} /> },
  { key: "finish", label: "Finish", icon: <Flag size={22} /> },
];

/** Settings offered during setup; everything else stays at the server default and can be changed later in System › Settings. */
export const ACCESS_SETTINGS = ["allow_registration", "refresh_token_ttl_days", "login_max_failures", "login_lockout_minutes", "ai_requests_per_hour"];
export const LOGGING_SETTINGS = ["log_level", "log_to_file", "log_retention_days", "log_console_format"];

export type SettingValue = boolean | number | string;
