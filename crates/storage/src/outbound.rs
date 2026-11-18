//! Outbound webhook endpoints and their deliveries (V0.8.4).

use super::*;
use repomemo_domain::{OutboundDelivery, OutboundEndpoint};

/// The events an endpoint may subscribe to. Payloads carry identifiers and counts, never titles or text.
pub const OUTBOUND_EVENTS: [&str; 5] = [
    "ping",
    "connector.sync_completed",
    "connector.sync_failed",
    "connector.quarantined",
    "connector.released",
];

/// Attempts made before a delivery is given up on.
pub const MAX_DELIVERY_ATTEMPTS: i64 = 5;
/// Deliveries kept per endpoint for inspection.
const MAX_DELIVERIES_KEPT: i64 = 200;
const MAX_ENDPOINTS_PER_WORKSPACE: i64 = 10;

fn secret_context(endpoint_id: &str) -> String {
    format!("outbound_endpoints:{endpoint_id}:secret")
}

/// Seconds to wait after the nth failed attempt: 30 s, 2 min, 10 min, 1 h.
pub fn retry_delay_seconds(attempts: i64) -> i64 {
    match attempts {
        1 => 30,
        2 => 120,
        3 => 600,
        _ => 3_600,
    }
}

/// A delivery that is due, with what is needed to send it.
#[derive(Debug, Clone)]
pub struct DueDelivery {
    pub id: String,
    pub endpoint_id: String,
    pub workspace_id: String,
    pub event_id: String,
    pub event_type: String,
    pub payload: String,
    pub attempts: i64,
    pub url: String,
    pub enabled: bool,
}

type EndpointRow = (
    String,
    String,
    String,
    String,
    String,
    i64,
    String,
    String,
    Option<String>,
);

fn endpoint_of(row: EndpointRow) -> OutboundEndpoint {
    OutboundEndpoint {
        id: row.0,
        workspace_id: row.1,
        name: row.2,
        url: row.3,
        events: serde_json::from_str(&row.4).unwrap_or_default(),
        enabled: row.5 != 0,
        created_at: row.6,
        updated_at: row.7,
        last_delivered_at: row.8,
    }
}

const ENDPOINT_COLUMNS: &str =
    "id, workspace_id, name, url, events_json, enabled, created_at, updated_at, last_delivered_at";

fn new_secret() -> Result<String> {
    use ring::rand::{SecureRandom, SystemRandom};
    let mut bytes = [0_u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("a signing secret could not be generated"))?;
    Ok(format!("whsec_{}", hex::encode(bytes)))
}

impl StorageEngine {
    /// Makes an endpoint and returns it with its signing secret, which is shown only now.
    pub async fn create_outbound_endpoint(
        &self,
        workspace_id: &str,
        name: &str,
        url: &str,
        events: &[String],
        actor_user_id: &str,
    ) -> Result<(OutboundEndpoint, String)> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbound_endpoints WHERE workspace_id = ?1")
            .bind(workspace_id)
            .fetch_one(self.reader())
            .await?;
        if count >= MAX_ENDPOINTS_PER_WORKSPACE {
            repomemo_domain::bail_conflict!(
                "A workspace can have {MAX_ENDPOINTS_PER_WORKSPACE} outbound endpoints. Remove one first."
            );
        }
        let id = Uuid::new_v4().to_string();
        let secret = new_secret()?;
        let sealed = self.secrets.seal(&secret, &secret_context(&id))?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT INTO outbound_endpoints
               (id, workspace_id, name, url, sealed_secret, events_json, enabled, created_by_user_id, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?8)"#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(name)
        .bind(url)
        .bind(sealed)
        .bind(serde_json::to_string(events)?)
        .bind(actor_user_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok((self.outbound_endpoint(&id).await?, secret))
    }

    pub async fn outbound_endpoint(&self, id: &str) -> Result<OutboundEndpoint> {
        let row = sqlx::query_as::<_, EndpointRow>(&format!(
            "SELECT {ENDPOINT_COLUMNS} FROM outbound_endpoints WHERE id = ?1"
        ))
        .bind(id)
        .fetch_optional(self.reader())
        .await?;
        match row {
            Some(row) => Ok(endpoint_of(row)),
            None => repomemo_domain::bail_not_found!("Endpoint was not found."),
        }
    }

    pub async fn list_outbound_endpoints(&self, workspace_id: &str) -> Result<Vec<OutboundEndpoint>> {
        Ok(sqlx::query_as::<_, EndpointRow>(&format!(
            "SELECT {ENDPOINT_COLUMNS} FROM outbound_endpoints WHERE workspace_id = ?1 ORDER BY created_at ASC"
        ))
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?
        .into_iter()
        .map(endpoint_of)
        .collect())
    }

    /// Changes the name, the events or whether it is on. Turning it off cancels what was still queued for it.
    pub async fn update_outbound_endpoint(
        &self,
        id: &str,
        name: Option<&str>,
        events: Option<&[String]>,
        enabled: Option<bool>,
    ) -> Result<OutboundEndpoint> {
        let current = self.outbound_endpoint(id).await?;
        let name = name.unwrap_or(&current.name);
        let events_json = serde_json::to_string(events.unwrap_or(&current.events))?;
        let enabled = enabled.unwrap_or(current.enabled);
        sqlx::query(
            "UPDATE outbound_endpoints SET name = ?2, events_json = ?3, enabled = ?4, updated_at = ?5 WHERE id = ?1",
        )
        .bind(id)
        .bind(name)
        .bind(events_json)
        .bind(i64::from(enabled))
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        if !enabled {
            sqlx::query(
                "UPDATE outbound_deliveries SET status = 'cancelled', next_attempt_at = NULL, last_error = 'The endpoint was turned off.' WHERE endpoint_id = ?1 AND status = 'pending'",
            )
            .bind(id)
            .execute(&self.pool)
            .await?;
        }
        self.outbound_endpoint(id).await
    }

    /// Makes a new secret, which ends the old one, and returns it once.
    pub async fn rotate_outbound_secret(&self, id: &str) -> Result<String> {
        self.outbound_endpoint(id).await?;
        let secret = new_secret()?;
        let sealed = self.secrets.seal(&secret, &secret_context(id))?;
        sqlx::query("UPDATE outbound_endpoints SET sealed_secret = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(id)
            .bind(sealed)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(secret)
    }

    pub async fn open_outbound_secret(&self, id: &str) -> Result<Option<String>> {
        let sealed = sqlx::query_scalar::<_, String>("SELECT sealed_secret FROM outbound_endpoints WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.reader())
            .await?;
        Ok(sealed.and_then(|sealed| self.secrets.open(&sealed, &secret_context(id)).ok()))
    }

    /// Removes an endpoint with its deliveries; nothing queued for it is sent.
    pub async fn delete_outbound_endpoint(&self, id: &str) -> Result<()> {
        self.outbound_endpoint(id).await?;
        sqlx::query("DELETE FROM outbound_endpoints WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Queues one event for every enabled endpoint of the workspace that subscribed to it. The event has one id for all of them.
    /// Returns the event id and how many endpoints will receive it.
    pub async fn enqueue_outbound_event(
        &self,
        workspace_id: &str,
        event_type: &str,
        data: &serde_json::Value,
    ) -> Result<(String, usize)> {
        let event_id = format!("evt_{}", Uuid::new_v4().simple());
        let payload = serde_json::json!({
            "id": event_id,
            "type": event_type,
            "created_at": Utc::now().to_rfc3339(),
            "workspace_id": workspace_id,
            "data": data,
        })
        .to_string();
        let mut queued = 0;
        for endpoint in self.list_outbound_endpoints(workspace_id).await? {
            if !endpoint.enabled || !endpoint.events.iter().any(|event| event == event_type) {
                continue;
            }
            self.queue_delivery(&endpoint.id, workspace_id, &event_id, event_type, &payload)
                .await?;
            queued += 1;
        }
        Ok((event_id, queued))
    }

    /// Queues a test event for one endpoint, whatever it subscribed to.
    pub async fn enqueue_outbound_ping(&self, endpoint_id: &str) -> Result<String> {
        let endpoint = self.outbound_endpoint(endpoint_id).await?;
        let event_id = format!("evt_{}", Uuid::new_v4().simple());
        let payload = serde_json::json!({
            "id": event_id,
            "type": "ping",
            "created_at": Utc::now().to_rfc3339(),
            "workspace_id": endpoint.workspace_id,
            "data": {},
        })
        .to_string();
        self.queue_delivery(&endpoint.id, &endpoint.workspace_id, &event_id, "ping", &payload)
            .await?;
        Ok(event_id)
    }

    async fn queue_delivery(
        &self,
        endpoint_id: &str,
        workspace_id: &str,
        event_id: &str,
        event_type: &str,
        payload: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT OR IGNORE INTO outbound_deliveries
               (id, endpoint_id, workspace_id, event_id, event_type, payload_json, status, attempts, next_attempt_at, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', 0, ?7, ?7)"#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(endpoint_id)
        .bind(workspace_id)
        .bind(event_id)
        .bind(event_type)
        .bind(payload)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            r#"DELETE FROM outbound_deliveries WHERE endpoint_id = ?1 AND id IN (
                 SELECT id FROM outbound_deliveries WHERE endpoint_id = ?1 ORDER BY created_at DESC LIMIT -1 OFFSET ?2)"#,
        )
        .bind(endpoint_id)
        .bind(MAX_DELIVERIES_KEPT)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Pending deliveries that are due, oldest first, with their endpoint's address and state.
    pub async fn due_outbound_deliveries(&self, limit: i64) -> Result<Vec<DueDelivery>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, String, i64, String, i64)>(
            r#"SELECT d.id, d.endpoint_id, d.workspace_id, d.event_id, d.event_type, d.payload_json, d.attempts,
                      e.url, e.enabled
               FROM outbound_deliveries d JOIN outbound_endpoints e ON e.id = d.endpoint_id
               WHERE d.status = 'pending' AND d.next_attempt_at <= ?1
               ORDER BY d.next_attempt_at ASC LIMIT ?2"#,
        )
        .bind(Utc::now().to_rfc3339())
        .bind(limit)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| DueDelivery {
                id: row.0,
                endpoint_id: row.1,
                workspace_id: row.2,
                event_id: row.3,
                event_type: row.4,
                payload: row.5,
                attempts: row.6,
                url: row.7,
                enabled: row.8 != 0,
            })
            .collect())
    }

    /// Records the outcome of an attempt. A success ends it; a failure waits and tries again until the attempts are used up.
    pub async fn record_outbound_attempt(
        &self,
        delivery: &DueDelivery,
        status_code: Option<u16>,
        error: Option<&str>,
    ) -> Result<()> {
        let attempts = delivery.attempts + 1;
        let now = Utc::now();
        let succeeded = status_code.is_some_and(|code| (200..300).contains(&code));
        let (status, next) = if succeeded {
            ("delivered", None)
        } else if attempts >= MAX_DELIVERY_ATTEMPTS {
            ("failed", None)
        } else {
            (
                "pending",
                Some((now + chrono::Duration::seconds(retry_delay_seconds(attempts))).to_rfc3339()),
            )
        };
        sqlx::query(
            r#"UPDATE outbound_deliveries SET status = ?2, attempts = ?3, next_attempt_at = ?4, last_status_code = ?5,
                 last_error = ?6, delivered_at = CASE WHEN ?2 = 'delivered' THEN ?7 ELSE delivered_at END
               WHERE id = ?1 AND status = 'pending'"#,
        )
        .bind(&delivery.id)
        .bind(status)
        .bind(attempts)
        .bind(next)
        .bind(status_code.map(i64::from))
        .bind(error.map(|text| text.chars().take(300).collect::<String>()))
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;
        if succeeded {
            sqlx::query("UPDATE outbound_endpoints SET last_delivered_at = ?2 WHERE id = ?1")
                .bind(&delivery.endpoint_id)
                .bind(now.to_rfc3339())
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Stops a delivery that must not be sent (its endpoint is off, or its address is no longer approved).
    pub async fn cancel_outbound_delivery(&self, delivery_id: &str, reason: &str) -> Result<()> {
        sqlx::query(
            "UPDATE outbound_deliveries SET status = 'cancelled', next_attempt_at = NULL, last_error = ?2 WHERE id = ?1 AND status = 'pending'",
        )
        .bind(delivery_id)
        .bind(reason)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The deliveries of an endpoint, newest first, without their payloads.
    pub async fn list_outbound_deliveries(&self, endpoint_id: &str, limit: i64) -> Result<Vec<OutboundDelivery>> {
        type Row = (
            String,
            String,
            String,
            String,
            i64,
            Option<String>,
            Option<i64>,
            Option<String>,
            String,
            Option<String>,
        );
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT id, event_id, event_type, status, attempts, next_attempt_at, last_status_code, last_error, created_at, delivered_at
               FROM outbound_deliveries WHERE endpoint_id = ?1 ORDER BY created_at DESC LIMIT ?2"#,
        )
        .bind(endpoint_id)
        .bind(limit.clamp(1, 200))
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| OutboundDelivery {
                id: row.0,
                event_id: row.1,
                event_type: row.2,
                status: row.3,
                attempts: row.4,
                next_attempt_at: row.5,
                last_status_code: row.6.map(|code| code as u16),
                last_error: row.7,
                created_at: row.8,
                delivered_at: row.9,
            })
            .collect())
    }
}
