import { useCallback, useEffect, useState } from "react";
import { listSharedWebhookDeliveries, resendSharedWebhookDelivery, type WebhookSourceKind } from "../lib/sharedApi";
import type { WebhookDelivery } from "../types";
import { apiMessage, formatTime } from "../web/format";
import { Button } from "./ui/button";
import { showToast } from "./ui/toast";

const OUTCOME_LABEL: Record<string, string> = {
  queued: "sync queued",
  absorbed: "a sync was already running",
  ignored: "ignored",
  pong: "answered the ping",
  failed: "could not be queued",
};

/** Only a call that asked for a sync can be sent again; a ping or an ignored event asked for nothing. */
export function canSendAgain(delivery: WebhookDelivery): boolean {
  return delivery.outcome === "queued" || delivery.outcome === "absorbed" || delivery.outcome === "failed";
}

/** One line for a call: the event and who sent it, what became of it and why not. */
export function describeDelivery(delivery: WebhookDelivery): string {
  const from = delivery.sender === "resend" ? "sent again from this page" : `from ${delivery.sender === "other" ? "another sender" : delivery.sender}`;
  const outcome = OUTCOME_LABEL[delivery.outcome] ?? delivery.outcome;
  return `${delivery.event} ${from} · ${outcome}${delivery.reason ? `: ${delivery.reason}` : ""}`;
}

/** The calls a connector's or a repository's webhook received, newest first, with a button to send one that asked
 *  for a sync again (V0.3.22). Shown to people who can write to the workspace. */
export function WebhookDeliveries({
  accessToken,
  kind,
  onResent,
  sourceId,
}: {
  accessToken: string;
  kind: WebhookSourceKind;
  onResent?: () => void;
  sourceId: string;
}) {
  const [deliveries, setDeliveries] = useState<WebhookDelivery[] | null>(null);
  const [sending, setSending] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setDeliveries(await listSharedWebhookDeliveries(accessToken, kind, sourceId));
    } catch (error) {
      showToast("error", apiMessage(error, "The calls could not be loaded."));
      setDeliveries((current) => current ?? []);
    }
  }, [accessToken, kind, sourceId]);
  useEffect(() => { void load(); }, [load]);

  async function sendAgain(delivery: WebhookDelivery) {
    setSending(delivery.id);
    try {
      const answer = await resendSharedWebhookDelivery(accessToken, kind, sourceId, delivery.id);
      if (answer.delivery.outcome === "failed") showToast("error", `It could not be queued: ${answer.note}`);
      else showToast("success", answer.queued ? "A sync was queued." : "A sync was already running.");
      onResent?.();
    } catch (error) {
      showToast("error", apiMessage(error, "The call could not be sent again."));
    } finally {
      setSending(null);
      await load();
    }
  }

  return <div className="shared-webhook-deliveries-panel">
    <strong>Recent calls</strong>
    {deliveries === null ? <span className="rm-map-muted">Loading…</span> : null}
    {deliveries?.length === 0 ? <span className="rm-map-muted">No call has been received yet.</span> : null}
    {deliveries?.length ? <ul className="shared-webhook-deliveries">
      {deliveries.map((delivery) => <li className="shared-webhook-delivery" key={delivery.id}>
        <span>
          <span className="rm-map-muted">{formatTime(delivery.received_at)} · </span>
          {describeDelivery(delivery)}
          {delivery.resent_at ? <span className="rm-map-muted"> · sent again {formatTime(delivery.resent_at)}</span> : null}
        </span>
        {canSendAgain(delivery) ? <Button
          aria-label={`Send again the ${delivery.event} call of ${formatTime(delivery.received_at)}`}
          busy={sending === delivery.id}
          disabled={sending !== null}
          onClick={() => void sendAgain(delivery)}
          variant="secondary"
        >Send again</Button> : null}
      </li>)}
    </ul> : null}
  </div>;
}
