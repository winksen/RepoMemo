import type { FormEvent } from "react";
import { useEffect, useState } from "react";
import { IconBrain as Brain, IconLoader2 as Loader, IconPhoto as Photo, IconRefresh as Refresh } from "@tabler/icons-react";
import { saveSharedWorkspaceAiProvider, testSharedWorkspaceAiProvider } from "../lib/sharedApi";
import type { ProviderTestResult, SharedAiProviderSettings } from "../types";
import { Button } from "./ui/button";
import { Dropdown } from "./ui/dropdown";
import { Input } from "./ui/input";
import { Toast } from "./ui/toast";

type ProviderType = "ollama" | "openrouter";
type Purpose = "text" | "vision";

const COPY: Record<Purpose, { title: string; intro: string; modelLabel: string; ollamaModel: string; cloudModel: string; cloudNotice: string }> = {
  text: {
    title: "AI for text",
    intro: "Answers, workspace overviews and summaries use this provider, grounded in cited evidence. Credentials are stored on the protected server and are never returned to the browser.",
    modelLabel: "Chat model",
    ollamaModel: "llama3.2",
    cloudModel: "openai/gpt-4o-mini",
    cloudNotice: "I understand that generating an overview sends cited workspace excerpts to this cloud provider.",
  },
  vision: {
    title: "AI for images",
    intro: "Uploaded images are turned into searchable text by this provider. Images cannot be uploaded until it is set up. Use a vision-capable model.",
    modelLabel: "Vision model",
    ollamaModel: "llama3.2-vision",
    cloudModel: "openai/gpt-4o-mini",
    cloudNotice: "I understand that uploaded images are sent to this cloud provider to be described.",
  },
};

const OLLAMA_URL = "http://127.0.0.1:11434";
const OPENROUTER_URL = "https://openrouter.ai/api/v1";

/** One provider configuration, either for text analysis or for image-to-text. */
export function AiProviderForm({
  accessToken,
  onSaved,
  providers,
  purpose,
  workspaceId,
}: {
  accessToken: string;
  onSaved: (provider: SharedAiProviderSettings) => void;
  providers: SharedAiProviderSettings[];
  purpose: Purpose;
  workspaceId: string;
}) {
  const copy = COPY[purpose];
  const [providerId, setProviderId] = useState("");
  const [providerType, setProviderType] = useState<ProviderType>("ollama");
  const [providerName, setProviderName] = useState(purpose === "vision" ? "Local Ollama vision" : "Local Ollama");
  const [baseUrl, setBaseUrl] = useState(OLLAMA_URL);
  const [model, setModel] = useState(copy.ollamaModel);
  const [apiKey, setApiKey] = useState("");
  const [acknowledged, setAcknowledged] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<ProviderTestResult | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  const own = providers.filter((entry) => entry.purpose === purpose);
  const current = own.find((entry) => entry.enabled) ?? own[0];

  useEffect(() => {
    if (!current) return;
    setProviderId(current.id);
    setProviderType(current.provider_type);
    setProviderName(current.name);
    setBaseUrl(current.base_url ?? "");
    setModel(current.model ?? "");
  }, [current?.id, current?.provider_type, current?.name, current?.base_url, current?.model]);

  function selectType(value: ProviderType) {
    setProviderType(value);
    if (value === "openrouter") {
      if (!baseUrl || baseUrl === OLLAMA_URL) setBaseUrl(OPENROUTER_URL);
      if (!model || model === copy.ollamaModel) setModel(copy.cloudModel);
    } else {
      if (!baseUrl || baseUrl === OPENROUTER_URL) setBaseUrl(OLLAMA_URL);
      if (!model || model === copy.cloudModel) setModel(copy.ollamaModel);
    }
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsBusy(true); setError(null);
    try {
      const saved = await saveSharedWorkspaceAiProvider(accessToken, workspaceId, {
        id: providerId || undefined,
        providerType,
        name: providerName,
        baseUrl,
        model,
        apiKey: apiKey || undefined,
        enabled: true,
        cloudContentAcknowledged: acknowledged,
        purpose,
      });
      setProviderId(saved.id);
      setApiKey("");
      setTestResult(null);
      onSaved(saved);
    } catch (requestError) {
      setError(requestError instanceof Error ? requestError.message : "The provider could not be saved.");
    } finally { setIsBusy(false); }
  }

  async function test() {
    if (!providerId) {
      setError("Save the provider before testing its connection.");
      return;
    }
    setIsBusy(true); setError(null); setTestResult(null);
    try { setTestResult(await testSharedWorkspaceAiProvider(accessToken, workspaceId, providerId)); }
    catch (requestError) { setError(requestError instanceof Error ? requestError.message : "The connection test failed."); }
    finally { setIsBusy(false); }
  }

  const Icon = purpose === "vision" ? Photo : Brain;
  return (
    <section className="shared-settings-group">
      <div className="shared-panel-heading"><div><Icon size={18} /><h2>{copy.title}</h2></div><span>{own.some((entry) => entry.enabled) ? "configured" : "not configured"}</span></div>
      <p className="shared-muted-copy">{copy.intro}</p>
      <form className="shared-ai-provider-form" onSubmit={save}>
        <label>Provider<Dropdown aria-label={`${copy.title} provider`} onValueChange={(value) => selectType(value as ProviderType)} options={[{ label: "Ollama (local)", value: "ollama" }, { label: "OpenRouter (cloud)", value: "openrouter" }]} value={providerType} /></label>
        <label>Provider name<Input onChange={(event) => setProviderName(event.target.value)} required value={providerName} /></label>
        <label>Base URL<Input onChange={(event) => setBaseUrl(event.target.value)} placeholder={providerType === "ollama" ? OLLAMA_URL : OPENROUTER_URL} value={baseUrl} /></label>
        <label>{copy.modelLabel}<Input onChange={(event) => setModel(event.target.value)} placeholder={providerType === "ollama" ? copy.ollamaModel : copy.cloudModel} required value={model} /></label>
        {providerType === "openrouter" ? <><label>API key<Input autoComplete="off" onChange={(event) => setApiKey(event.target.value)} placeholder={providerId ? "Leave blank to keep the saved key" : "Required to enable cloud AI"} type="password" value={apiKey} /></label><label className="shared-ai-provider-toggle"><input checked={acknowledged} onChange={(event) => setAcknowledged(event.target.checked)} type="checkbox" /> {copy.cloudNotice}</label></> : null}
        <div className="shared-ai-provider-actions"><Button disabled={isBusy} type="submit" variant="secondary">{isBusy ? <Loader className="spin" size={16} /> : <Icon size={16} />} Save provider</Button><Button disabled={isBusy || !providerId} onClick={() => void test()} type="button" variant="secondary">{isBusy ? <Loader className="spin" size={16} /> : <Refresh size={16} />} Test connection</Button></div>
        <Toast kind="error" message={error} />
        <Toast kind={testResult?.success ? "success" : "error"} message={testResult?.message} />
      </form>
    </section>
  );
}
