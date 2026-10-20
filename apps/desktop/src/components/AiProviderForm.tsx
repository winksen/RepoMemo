import type { FormEvent } from "react";
import { useEffect, useState } from "react";
import { IconBrain as Brain, IconPhoto as Photo, IconRefresh as Refresh, IconSearch as Search } from "@tabler/icons-react";
import { saveSharedWorkspaceAiProvider, testSharedWorkspaceAiProvider } from "../lib/sharedApi";
import type { ProviderTestResult, SharedAiProviderSettings } from "../types";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import { PanelHeading } from "./ui/panel-heading";
import { Dropdown } from "./ui/dropdown";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { Toast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage } from "../web/format";

type ProviderType = "ollama" | "openrouter";
type Purpose = "text" | "vision" | "embedding";

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
  embedding: {
    title: "AI for search",
    intro: "Builds search vectors so search, the assistant and answers also find passages by meaning, not only by matching words. Every indexed passage is embedded in the background. Use an embedding model; multilingual models such as bge-m3 handle French and English.",
    modelLabel: "Embedding model",
    ollamaModel: "bge-m3",
    cloudModel: "baai/bge-m3",
    cloudNotice: "I understand that the text of every indexed file, not only cited excerpts, is sent to this cloud provider to build search vectors.",
  },
};

const DEFAULT_NAMES: Record<Purpose, string> = {
  text: "Local Ollama",
  vision: "Local Ollama vision",
  embedding: "Local Ollama embeddings",
};

const ICONS = { text: Brain, vision: Photo, embedding: Search } as const;

const OLLAMA_URL = "http://127.0.0.1:11434";
const OPENROUTER_URL = "https://openrouter.ai/api/v1";

/** One provider configuration, either for text analysis or for image-to-text. */
export function AiProviderForm({
  accessToken,
  onSaved,
  providers,
  purpose,
  status,
  workspaceId,
}: {
  accessToken: string;
  onSaved: (provider: SharedAiProviderSettings) => void;
  providers: SharedAiProviderSettings[];
  purpose: Purpose;
  /** Extra live state shown under the description, such as embedding progress. */
  status?: string | null;
  workspaceId: string;
}) {
  const copy = COPY[purpose];
  const [providerId, setProviderId] = useState("");
  const [providerType, setProviderType] = useState<ProviderType>("ollama");
  const [providerName, setProviderName] = useState(DEFAULT_NAMES[purpose]);
  const [baseUrl, setBaseUrl] = useState(OLLAMA_URL);
  const [model, setModel] = useState(copy.ollamaModel);
  const [apiKey, setApiKey] = useState("");
  const [acknowledged, setAcknowledged] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<ProviderTestResult | null>(null);
  const [action, setAction] = useState<"save" | "test" | null>(null);
  const isBusy = action !== null;

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
    setAction("save"); setError(null);
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
      setError(apiMessage(requestError, "The provider could not be saved."));
    } finally { setAction(null); }
  }

  async function test() {
    if (!providerId) {
      setError("Save the provider before testing its connection.");
      return;
    }
    setAction("test"); setError(null); setTestResult(null);
    try { setTestResult(await testSharedWorkspaceAiProvider(accessToken, workspaceId, providerId)); }
    catch (requestError) { setError(apiMessage(requestError, "The connection test failed.")); }
    finally { setAction(null); }
  }

  const Icon = ICONS[purpose];
  return (
    <section className="shared-settings-group">
      <PanelHeading description={own.some((entry) => entry.enabled) ? "configured" : "not configured"} icon={Icon} title={copy.title} />
      <p className="shared-muted-copy">{copy.intro}</p>
      {status ? <p className="shared-ai-provider-status">{status}</p> : null}
      <form className="shared-ai-provider-form" onSubmit={save}>
        <Field label="Provider">{(control) => <Dropdown aria-label={`${copy.title} provider`} id={control.id} onValueChange={(value) => selectType(value as ProviderType)} options={[{ label: "Ollama (local)", value: "ollama" }, { label: "OpenRouter (cloud)", value: "openrouter" }]} value={providerType} />}</Field>
        <Field label="Provider name" required><Input onChange={(event) => setProviderName(event.target.value)} value={providerName} /></Field>
        <Field label="Base URL"><Input onChange={(event) => setBaseUrl(event.target.value)} placeholder={providerType === "ollama" ? OLLAMA_URL : OPENROUTER_URL} value={baseUrl} /></Field>
        <Field label={copy.modelLabel} required><Input onChange={(event) => setModel(event.target.value)} placeholder={providerType === "ollama" ? copy.ollamaModel : copy.cloudModel} value={model} /></Field>
        {providerType === "openrouter" ? <><Field label="API key"><Input autoComplete="off" onChange={(event) => setApiKey(event.target.value)} placeholder={providerId ? "Leave blank to keep the saved key (enter it again if you change the address)" : "Required to enable cloud AI"} type="password" value={apiKey} /></Field><Checkbox checked={acknowledged} label={copy.cloudNotice} onCheckedChange={setAcknowledged} /></> : null}
        <div className="shared-ai-provider-actions"><Button busy={action === "save"} disabled={isBusy} type="submit" variant="secondary"><Icon size={ICON_SIZE.md} /> Save provider</Button><Button busy={action === "test"} disabled={isBusy || !providerId} onClick={() => void test()} variant="secondary"><Refresh size={ICON_SIZE.md} /> Test connection</Button></div>
        <Toast kind="error" message={error} />
        <Toast kind={testResult?.success ? "success" : "error"} message={testResult?.message} />
      </form>
    </section>
  );
}
