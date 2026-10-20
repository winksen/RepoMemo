import { useMemo, useState } from "react";
import { MailSettingsPanel } from "../../components/MailSettingsPanel";
import {
  IconAdjustments as Adjustments,
  IconAlertCircle as AlertCircle,
  IconCircleCheck as CircleCheck,
  IconRotate as Rotate,
  IconServer as Server,
  IconPlugConnectedX as Unplug,
} from "@tabler/icons-react";
import {
  detachEnvironment,
  getPlatformStatus,
  getSystemSettings,
  clearSharedSession,
  resetSystemSetting,
  saveSystemSettings,
  PlatformStatus,
} from "../../lib/sharedApi";
import type { SharedSession, SystemSetting, SystemSettings } from "../../types";
import { Button } from "../../components/ui/button";
import { PanelHeading } from "../../components/ui/panel-heading";
import { Dialog, DialogCancel } from "../../components/ui/dialog";
import { LoadingState } from "../../components/ui/loading-state";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage, formatTime } from "../format";
import { Facts, useSystemData } from "./shared";
import { SettingControl, settingProblem } from "./SettingControl";
import { systemKeys } from "../../lib/queryClient";
import { useServerState } from "../../lib/useServerState";

/** The environment folder this server is attached to, and the way to detach it (system administrators only). */
function EnvironmentGroup({ accessToken }: { accessToken: string }) {
  const [platform] = useServerState<PlatformStatus | null>({ key: systemKeys.platform(), fetch: getPlatformStatus, initial: null, staleTime: 0 });
  const [isConfirming, setIsConfirming] = useState(false);
  const [isBusy, setIsBusy] = useState(false);

  async function detach() {
    setIsBusy(true);
    try {
      await detachEnvironment(accessToken);
      // Every session ended on the server; start over from the environment menu.
      clearSharedSession();
      window.location.assign("/");
    } catch (error) {
      showToast("error", apiMessage(error, "The environment could not be detached."));
      setIsBusy(false);
    }
  }

  return <section className="shared-settings-group rm-environment-card">
    <PanelHeading description={platform?.environment ?? "—"} icon={Unplug} title="Environment" />
    <p className="shared-muted-copy">All data of this server lives in <code>{platform ? `${platform.folder}/${platform.environment ?? ""}` : "its environment folder"}</code>. Detaching signs everyone out, stops background work and closes the database. The server then waits for an environment to be chosen again, using the code printed on its console. Nothing is deleted.</p>
    {platform?.pinned_environment
      ? <p className="rm-platform-note" role="note"><AlertCircle size={ICON_SIZE.md} /><span>This server starts with <code>{platform.pinned_environment}</code>, set by <code>REPOMEMO_SERVER_DATA_DIR</code>. Another environment chosen after detaching is used only until the server restarts, then it goes back to <code>{platform.pinned_environment}</code>.</span></p>
      : <p className="rm-platform-note" role="note"><CircleCheck size={ICON_SIZE.md} /><span>This server remembers the environment chosen in the web app and attaches it again when it restarts. After detaching, the next start shows the environment menu until one is chosen.</span></p>}
    <div><Button disabled={isBusy || !platform?.environment} onClick={() => setIsConfirming(true)} variant="secondary"><Unplug size={ICON_SIZE.md} /> Detach environment</Button></div>
    <Dialog description={<>Everyone, you included, is signed out at once and the server stops serving this environment until an administrator with access to the server console attaches one again.{platform?.pinned_environment ? <> <strong>The switch lasts only until the next restart:</strong> this server starts with <code>{platform.pinned_environment}</code> (<code>REPOMEMO_SERVER_DATA_DIR</code>), so a restart goes back to it whatever is chosen now.</> : <> The environment chosen next is remembered for later restarts.</>}</>} footer={<><DialogCancel onClick={() => setIsConfirming(false)} /><Button busy={isBusy} onClick={() => void detach()} variant="danger">Detach environment</Button></>} onClose={() => setIsConfirming(false)} open={isConfirming} title={`Detach ${platform?.environment ?? "the environment"}?`} />
  </section>;
}

export function SettingsSection({ accessToken, session }: { accessToken: string; session: SharedSession }) {
  const [settings, , , setSettings] = useSystemData<SystemSettings>(systemKeys.settings(), () => getSystemSettings(accessToken), { failure: "Settings could not be loaded." });
  const [draft, setDraft] = useState<Record<string, boolean | number | string>>({});
  const [isBusy, setIsBusy] = useState(false);

  const groups = useMemo(() => {
    const byGroup = new Map<string, SystemSetting[]>();
    for (const setting of settings?.settings ?? []) byGroup.set(setting.group, [...(byGroup.get(setting.group) ?? []), setting]);
    return [...byGroup.entries()];
  }, [settings]);
  const changed = Object.keys(draft).length;
  // A whole number outside its range cannot be saved.
  const hasProblem = (settings?.settings ?? []).some((setting) => setting.key in draft && settingProblem(setting, draft[setting.key]) !== null);

  function edit(setting: SystemSetting, value: boolean | number | string) {
    setDraft((current) => {
      const next = { ...current };
      if (value === setting.value) delete next[setting.key];
      else next[setting.key] = value;
      return next;
    });
  }

  async function save() {
    setIsBusy(true);
    try {
      setSettings(await saveSystemSettings(accessToken, draft));
      setDraft({});
      showToast("success", `${changed} setting${changed === 1 ? "" : "s"} saved. They apply now.`);
    } catch (error) { showToast("error", apiMessage(error, "The settings could not be saved.")); }
    finally { setIsBusy(false); }
  }

  async function reset(setting: SystemSetting) {
    setIsBusy(true);
    try {
      setSettings(await resetSystemSetting(accessToken, setting.key));
      setDraft((current) => { const next = { ...current }; delete next[setting.key]; return next; });
      showToast("success", `${setting.label} is back to the server default.`);
    } catch (error) { showToast("error", apiMessage(error, "The setting could not be reset.")); }
    finally { setIsBusy(false); }
  }

  if (!settings) return <LoadingState label="Loading settings" />;
  // App administrators read the settings; system administrators change them.
  const readOnly = !session.is_system_admin;
  return <>
    {readOnly ? <p className="shared-muted-copy">Read only: only system administrators change these settings.</p> : <div className="rm-system-toolbar"><span className="shared-muted-copy">{changed ? `${changed} unsaved change${changed === 1 ? "" : "s"}` : "No unsaved changes"}</span><div className="rm-system-actions"><Button disabled={!changed || isBusy} onClick={() => setDraft({})} variant="secondary">Discard</Button><Button busy={isBusy} disabled={!changed || hasProblem} onClick={() => void save()} variant="main">Save changes</Button></div></div>}
    {groups.map(([group, entries]) => <section className="shared-settings-group" key={group}>
      <PanelHeading icon={Adjustments} title={group} />
      <div className="rm-system-settings">{entries.map((setting) => {
        const value = draft[setting.key] ?? setting.value;
        const inputId = `system-setting-${setting.key}`;
        return <div className="rm-system-setting" key={setting.key}>
          <div><label htmlFor={inputId}><strong>{setting.label}</strong></label><p id={`${inputId}-description`}>{setting.description}</p>
            <small>Server default: {settingValueLabel(setting, setting.default_value)}{setting.overridden ? ` · changed${setting.updated_by ? ` by ${setting.updated_by}` : ""} ${formatTime(setting.updated_at)}` : ""}</small>
          </div>
          <div className="rm-system-setting-control">
            <SettingControl describedBy={`${inputId}-description`} disabled={readOnly} id={inputId} onChange={(next) => edit(setting, next)} setting={setting} value={value} />
            {setting.overridden && !readOnly ? <Button disabled={isBusy} onClick={() => void reset(setting)} variant="secondary"><Rotate size={ICON_SIZE.sm} /> Use default</Button> : null}
          </div>
        </div>;
      })}</div>
    </section>)}
    <MailSettingsPanel accessToken={accessToken} canChange={session.is_system_admin === true} />
    {session.is_system_admin ? <EnvironmentGroup accessToken={accessToken} /> : null}
    <section className="shared-settings-group">
      <PanelHeading description="read only" icon={Server} title="Set by the server environment" />
      <p className="shared-muted-copy">These are chosen by whoever runs the server, through environment variables, and need a restart to change.</p>
      <Facts items={settings.environment.map((fact) => [fact.label, fact.value])} />
    </section>
  </>;
}

function settingValueLabel(setting: SystemSetting, value: boolean | number | string): string {
  if (setting.kind === "boolean") return value ? "on" : "off";
  if (setting.kind === "choice") return setting.options.find((option) => option.value === value)?.label ?? String(value);
  return `${value}${setting.unit ? ` ${setting.unit}` : ""}`;
}
