import { useEffect, useState } from "react";
import { IconShieldLock as ShieldLock } from "@tabler/icons-react";
import { getSharedAiPolicy, saveSharedAiPolicy } from "../lib/sharedApi";
import type { AiMinRole, AiPolicy } from "../types";
import { Dropdown } from "./ui/dropdown";
import { Toast } from "./ui/toast";

const OPTIONS: { label: string; value: AiMinRole }[] = [
  { label: "Everyone in the workspace", value: "viewer" },
  { label: "Members, administrators and owners", value: "member" },
  { label: "Administrators and owners", value: "admin" },
];

/** Which roles may trigger AI features in a workspace. Shown to owners and administrators. */
export function AiPolicySettings({ accessToken, onSaved, workspaceId }: {
  accessToken: string;
  /** Called after the policy changed, so capabilities can be reloaded. */
  onSaved?: (policy: AiPolicy) => void;
  workspaceId: string;
}) {
  const [policy, setPolicy] = useState<AiPolicy | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setPolicy(null);
    getSharedAiPolicy(accessToken, workspaceId)
      .then((next) => { if (active) setPolicy(next); })
      .catch((requestError) => { if (active) setError(requestError instanceof Error ? requestError.message : "The AI policy could not be loaded."); });
    return () => { active = false; };
  }, [accessToken, workspaceId]);

  async function change(value: string) {
    setIsSaving(true); setError(null); setNotice(null);
    try {
      const saved = await saveSharedAiPolicy(accessToken, workspaceId, value as AiMinRole);
      setPolicy(saved);
      setNotice("AI access updated.");
      onSaved?.(saved);
    } catch (requestError) {
      setError(requestError instanceof Error ? requestError.message : "The AI policy could not be saved.");
    } finally { setIsSaving(false); }
  }

  const quota = policy ? (policy.requests_per_hour > 0 ? `${policy.requests_per_hour} AI requests per person per hour` : "no hourly limit") : "loading";
  return (
    <section className="shared-settings-group">
      <div className="shared-panel-heading"><div><ShieldLock size={18} /><h2>Who can use AI</h2></div><span>{quota}</span></div>
      <p className="shared-muted-copy">Answers, overviews, summaries and the assistant's AI actions send cited excerpts to the workspace's text provider. Choose which roles may start them. Image descriptions and search vectors are built in the background for everyone and are not affected.</p>
      <form className="shared-ai-provider-form" onSubmit={(event) => event.preventDefault()}>
        <label>AI features available to<Dropdown aria-label="Roles that can use AI features" disabled={!policy || isSaving} onValueChange={(value) => void change(value)} options={OPTIONS} value={policy?.min_role} /></label>
        <Toast kind="error" message={error} />
        <Toast kind="success" message={notice} />
      </form>
    </section>
  );
}
