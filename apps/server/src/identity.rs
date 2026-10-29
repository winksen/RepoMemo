//! One OIDC provider per server. All egress is HTTPS, same-origin and DNS-pinned.
//! Provider errors never include response bodies, request URLs or credentials.
use crate::*;
use repomemo_storage::IdentityProvider;
use serde_json::{json, Value};
use url::Url;

const MAX_DOCUMENT: usize = 256 * 1024;

#[derive(Deserialize)]
pub(crate) struct ProviderRequest {
    name: String,
    issuer: String,
    client_id: String,
    #[serde(default)]
    client_secret: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    allowed_domains: Vec<String>,
}

fn https_url(value: &str) -> Result<Url, ApiError> {
    let url = Url::parse(value).map_err(|_| ApiError::bad_request("Enter an absolute HTTPS provider address."))?;
    if value.len() > 2048
        || url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ApiError::bad_request(
            "Provider addresses require HTTPS, without credentials, query or fragment.",
        ));
    }
    Ok(url)
}

/// Resolve once, reject non-public addresses, and connect only to the checked IPs.
/// Disabling proxies also prevents an environment proxy from bypassing this guard.
async fn http_client(url: &Url) -> Result<reqwest::Client, ApiError> {
    let host = url
        .host_str()
        .ok_or_else(|| ApiError::bad_request("Provider host is missing."))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses: Vec<_> = tokio::time::timeout(Duration::from_secs(5), tokio::net::lookup_host((host, port)))
        .await
        .map_err(|_| ApiError::bad_request("Provider DNS lookup timed out."))?
        .map_err(|_| ApiError::bad_request("Provider host could not be resolved."))?
        .collect();
    if addresses.is_empty()
        || addresses.iter().any(|a| repomemo_ai::is_non_public_address(&a.ip()))
        || repomemo_ai::is_metadata_host(host)
    {
        return Err(ApiError::bad_request(
            "The identity provider must resolve only to public addresses.",
        ));
    }
    reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .resolve_to_addrs(host, &addresses)
        .build()
        .map_err(|_| ApiError::bad_request("The provider connection could not be prepared."))
}

async fn bounded_json(mut response: reqwest::Response) -> Result<Value, ApiError> {
    if !response.status().is_success() {
        return Err(ApiError::bad_request(
            "The provider refused the request. Check its configuration and availability.",
        ));
    }
    if response.content_length().is_some_and(|n| n > MAX_DOCUMENT as u64) {
        return Err(ApiError::bad_request("The provider document is too large."));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ApiError::bad_request("The provider response was interrupted."))?
    {
        if bytes.len() + chunk.len() > MAX_DOCUMENT {
            return Err(ApiError::bad_request("The provider document is too large."));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| ApiError::bad_request("The provider returned an invalid JSON document."))
}

async fn fetch_json(address: &str) -> Result<Value, ApiError> {
    let url = https_url(address)?;
    let response = http_client(&url).await?.get(url).send().await.map_err(|_| {
        ApiError::bad_request(
            "The provider could not be reached securely. Check its address, certificate and availability.",
        )
    })?;
    bounded_json(response).await
}

fn endpoint(document: &Value, key: &str, issuer: &Url) -> Result<String, ApiError> {
    let value = document[key]
        .as_str()
        .ok_or_else(|| ApiError::bad_request(format!("Discovery is missing {key}.")))?;
    let url = https_url(value)?;
    if url.origin() != issuer.origin() {
        return Err(ApiError::bad_request(
            "Provider endpoints must share the issuer's HTTPS origin.",
        ));
    }
    Ok(value.to_owned())
}

fn discovery(provider: &mut IdentityProvider, document: &Value) -> Result<(), ApiError> {
    let issuer = https_url(&provider.issuer)?;
    if document["issuer"].as_str() != Some(provider.issuer.as_str()) {
        return Err(ApiError::bad_request(
            "Discovery issuer does not exactly match the configured issuer.",
        ));
    }
    let supports = |key: &str, value: &str| document[key].as_array().is_some_and(|a| a.iter().any(|v| v == value));
    if !supports("response_types_supported", "code")
        || !supports("id_token_signing_alg_values_supported", "RS256")
        || !supports("code_challenge_methods_supported", "S256")
        || !supports("token_endpoint_auth_methods_supported", "client_secret_post")
    {
        return Err(ApiError::bad_request(
            "The provider must advertise code flow, RS256, S256 PKCE and client_secret_post.",
        ));
    }
    provider.authorization_endpoint = endpoint(document, "authorization_endpoint", &issuer)?;
    provider.token_endpoint = endpoint(document, "token_endpoint", &issuer)?;
    provider.jwks_uri = endpoint(document, "jwks_uri", &issuer)?;
    provider.end_session_endpoint = if document["end_session_endpoint"].is_string() {
        Some(endpoint(document, "end_session_endpoint", &issuer)?)
    } else {
        None
    };
    provider.checked_at = Some(Utc::now().to_rfc3339());
    Ok(())
}

fn configuration(request: &ProviderRequest) -> Result<IdentityProvider, ApiError> {
    https_url(&request.issuer)?;
    if request.name.trim().is_empty()
        || request.name.len() > 80
        || request.client_id.trim().is_empty()
        || request.client_id.len() > 512
        || request.client_secret.len() > 16384
        || request.allowed_domains.len() > 50
        || request.allowed_domains.iter().any(|d| {
            d.is_empty()
                || d.len() > 253
                || !d.contains('.')
                || d.bytes().any(|c| !c.is_ascii_alphanumeric() && c != b'.' && c != b'-')
        })
    {
        return Err(ApiError::bad_request(
            "Enter a provider name, client ID and exact email domains within the allowed limits.",
        ));
    }
    Ok(IdentityProvider {
        name: request.name.trim().into(),
        issuer: request.issuer.clone(),
        client_id: request.client_id.trim().into(),
        enabled: request.enabled,
        allowed_domains: request.allowed_domains.iter().map(|d| d.to_ascii_lowercase()).collect(),
        ..Default::default()
    })
}

pub(crate) async fn get_provider(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    system::require_full_system_admin(&subject)?;
    let provider = state.storage.identity_provider().await.map_err(ApiError::internal)?;
    Ok(Json(
        json!({"provider": provider, "has_secret": state.storage.identity_secret().await.map_err(ApiError::internal)?.is_some(), "callback_url": state.mail.link("/v1/auth/identity/callback")}),
    ))
}

pub(crate) async fn check_provider(
    SessionSubject(subject): SessionSubject,
    State(_state): State<AppState>,
    Json(request): Json<ProviderRequest>,
) -> Result<Json<Value>, ApiError> {
    system::require_full_system_admin(&subject)?;
    let mut provider = configuration(&request)?;
    let document = fetch_json(&format!(
        "{}/.well-known/openid-configuration",
        provider.issuer.trim_end_matches('/')
    ))
    .await?;
    discovery(&mut provider, &document)?;
    let keys = fetch_json(&provider.jwks_uri).await?;
    if !keys["keys"]
        .as_array()
        .is_some_and(|keys| !keys.is_empty() && keys.len() <= 32)
    {
        return Err(ApiError::bad_request("The provider key set is empty or too large."));
    }
    Ok(Json(
        json!({"message":"Discovery and signing-key retrieval passed. This does not yet prove the client credentials or sign-in.","provider":provider}),
    ))
}

pub(crate) async fn save_provider(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Json(request): Json<ProviderRequest>,
) -> Result<Json<Value>, ApiError> {
    system::require_full_system_admin(&subject)?;
    let mut provider = configuration(&request)?;
    let old = state.storage.identity_provider().await.map_err(ApiError::internal)?;
    let secret = if request.client_secret.is_empty() {
        if old
            .as_ref()
            .is_some_and(|p| p.issuer != provider.issuer || p.client_id != provider.client_id)
        {
            return Err(ApiError::bad_request(
                "Enter a new secret when changing the issuer or client ID.",
            ));
        }
        state
            .storage
            .identity_secret()
            .await
            .map_err(ApiError::internal)?
            .unwrap_or_default()
    } else {
        request.client_secret
    };
    if secret.is_empty() {
        return Err(ApiError::bad_request("Enter the provider client secret."));
    }
    if provider.enabled {
        if !state
            .mail
            .link("/v1/auth/identity/callback")
            .is_some_and(|s| s.starts_with("https://"))
        {
            return Err(ApiError::bad_request(
                "Configure an HTTPS public server URL before enabling provider sign-in.",
            ));
        }
        let document = fetch_json(&format!(
            "{}/.well-known/openid-configuration",
            provider.issuer.trim_end_matches('/')
        ))
        .await?;
        discovery(&mut provider, &document)?;
        let keys = fetch_json(&provider.jwks_uri).await?;
        if !keys["keys"].as_array().is_some_and(|a| !a.is_empty() && a.len() <= 32) {
            return Err(ApiError::bad_request("The provider key set is empty or too large."));
        }
    }
    state
        .storage
        .save_identity_provider(&provider, &secret, &subject.user_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(
        json!({"provider":provider,"has_secret":true,"callback_url":state.mail.link("/v1/auth/identity/callback")}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_urls_refuse_insecure_or_ambiguous_addresses() {
        for url in [
            "http://id.example.com",
            "https://u:p@id.example.com",
            "https://id.example.com?x=y",
            "https://id.example.com/#a",
        ] {
            assert!(https_url(url).is_err());
        }
    }
    #[test]
    fn identity_discovery_rejects_mixup_and_cross_origin_endpoints() {
        let mut provider = IdentityProvider {
            issuer: "https://id.example.com/realm".into(),
            ..Default::default()
        };
        let mut doc = json!({"issuer":provider.issuer,"response_types_supported":["code"],"id_token_signing_alg_values_supported":["RS256"],"code_challenge_methods_supported":["S256"],"token_endpoint_auth_methods_supported":["client_secret_post"],"authorization_endpoint":"https://id.example.com/auth","token_endpoint":"https://id.example.com/token","jwks_uri":"https://id.example.com/keys"});
        assert!(discovery(&mut provider, &doc).is_ok());
        doc["token_endpoint"] = json!("https://other.example.com/token");
        assert!(discovery(&mut provider, &doc).is_err());
        doc["issuer"] = json!("https://other.example.com");
        assert!(discovery(&mut provider, &doc).is_err());
    }
}
