//! One OIDC provider per server. All egress is HTTPS, same-origin and DNS-pinned.
//! Provider errors never include response bodies, request URLs or credentials.
use crate::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use repomemo_storage::IdentityProvider;
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{json, Value};
use url::Url;

fn random_secret() -> Result<String, ApiError> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| ApiError::internal("Random generation failed"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes()).as_ref())
}

async fn enabled_provider(state: &AppState) -> Result<IdentityProvider, ApiError> {
    state
        .storage
        .identity_provider()
        .await
        .map_err(ApiError::internal)?
        .filter(|p| p.enabled && p.checked_at.is_some())
        .ok_or_else(|| {
            ApiError::bad_request(
                "Provider sign-in is unavailable. Use the local sign-in path or contact an administrator.",
            )
        })
}

pub(crate) async fn public_provider(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let provider = state.storage.identity_provider().await.map_err(ApiError::internal)?;
    let provider = provider.filter(|p| p.enabled);
    Ok(Json(
        json!({"name":provider.as_ref().map(|p|&p.name),"enforced":provider.is_some_and(|p|p.enforced)}),
    ))
}

/// The name of the provider when password sign-in is replaced by it.
pub(crate) async fn enforced_provider(state: &AppState) -> Result<Option<String>, ApiError> {
    Ok(state
        .storage
        .identity_provider()
        .await
        .map_err(ApiError::internal)?
        .filter(|p| p.enabled && p.enforced)
        .map(|p| p.name))
}

/// Password routes consult this after the password is proven. Only break-glass system administrators pass while
/// enforcement is on; if none of them is left usable, any full system administrator does, so a path always remains.
pub(crate) async fn password_sign_in_allowed(state: &AppState, user_id: &str) -> Result<(), ApiError> {
    let Some(provider) = state
        .storage
        .identity_provider()
        .await
        .map_err(ApiError::internal)?
        .filter(|p| p.enabled && p.enforced)
    else {
        return Ok(());
    };
    let mut usable = Vec::new();
    for id in &provider.break_glass {
        let access = state.storage.user_access(id).await.map_err(ApiError::internal)?;
        if access.is_some_and(|a| a.is_system_admin) {
            usable.push(id.as_str());
        }
    }
    let is_admin = state
        .storage
        .user_access(user_id)
        .await
        .map_err(ApiError::internal)?
        .is_some_and(|a| a.is_system_admin);
    if !is_admin || (!usable.is_empty() && !usable.contains(&user_id)) {
        return Err(ApiError::forbidden_because(format!(
            "This server requires sign-in with {}.",
            provider.name
        )));
    }
    state
        .storage
        .record_system_event(
            Some(user_id),
            "break_glass_sign_in",
            "Password sign-in used while provider sign-in is enforced",
        )
        .await
        .map_err(ApiError::internal)?;
    Ok(())
}

#[derive(Default, Deserialize)]
pub(crate) struct StartRequest {
    #[serde(default)]
    invitation_token: Option<String>,
}

pub(crate) async fn links(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"links":state.storage.identity_links(&subject.user_id).await.map_err(ApiError::internal)?}),
    ))
}

pub(crate) async fn start_link(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    client: ClientIp,
) -> Result<Json<Value>, ApiError> {
    begin(
        &state,
        &client,
        json!({"link_user":subject.user_id,"link_version":subject.session_version}),
    )
    .await
}

pub(crate) async fn start(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<StartRequest>,
) -> Result<Json<Value>, ApiError> {
    if request.invitation_token.as_ref().is_some_and(|s| s.len() > 512) {
        return Err(ApiError::bad_request("Invalid invitation."));
    }
    begin(&state, &client, json!({"invitation_token":request.invitation_token})).await
}

async fn begin(state: &AppState, client: &ClientIp, context: Value) -> Result<Json<Value>, ApiError> {
    auth::check_auth_quota(state, client, auth::AuthAction::SignIn)?;
    if setup::setup_pending(state).await? {
        return Err(ApiError::forbidden());
    }
    let provider = enabled_provider(state).await?;
    let callback = state
        .mail
        .link("/v1/auth/identity/callback")
        .ok_or_else(|| ApiError::bad_request("The public server URL is missing."))?;
    let state_value = random_secret()?;
    let proof = random_secret()?;
    let nonce = random_secret()?;
    let verifier = random_secret()?;
    let mut url = https_url(&provider.authorization_endpoint)?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", provider.client_id.as_str()),
        ("response_type", "code"),
        ("scope", "openid email profile"),
        ("redirect_uri", callback.as_str()),
        ("state", state_value.as_str()),
        ("nonce", nonce.as_str()),
        ("code_challenge", challenge(&verifier).as_str()),
        ("code_challenge_method", "S256"),
        ("max_age", "0"),
    ]);
    state
        .storage
        .create_identity_flow(
            &state_value,
            &proof,
            &json!({"provider":provider,"callback":callback,"nonce":nonce,"verifier":verifier,"context":context}),
        )
        .await
        .map_err(map_storage_error)?;
    Ok(Json(
        json!({"authorization_url":url.as_str(),"state":state_value,"proof":proof,"expires_in":300}),
    ))
}

#[derive(Deserialize)]
pub(crate) struct Callback {
    state: String,
    code: Option<String>,
    error: Option<String>,
}

fn token_claims(token: &str, keys: &Value, provider: &IdentityProvider, nonce: &str) -> Result<Value, ApiError> {
    let invalid = || ApiError::bad_request("The provider identity token could not be verified. Start sign-in again.");
    let header = jsonwebtoken::decode_header(token).map_err(|_| invalid())?;
    if header.alg != Algorithm::RS256 {
        return Err(invalid());
    }
    let kid = header.kid.ok_or_else(invalid)?;
    let keys: jsonwebtoken::jwk::JwkSet = serde_json::from_value(keys.clone()).map_err(|_| invalid())?;
    if keys
        .keys
        .iter()
        .filter(|k| k.common.key_id.as_ref() == Some(&kid))
        .count()
        != 1
    {
        return Err(invalid());
    }
    let key = keys.find(&kid).ok_or_else(invalid)?;
    if key
        .common
        .public_key_use
        .as_ref()
        .is_some_and(|u| *u != jsonwebtoken::jwk::PublicKeyUse::Signature)
        || key
            .common
            .key_algorithm
            .as_ref()
            .is_some_and(|a| *a != jsonwebtoken::jwk::KeyAlgorithm::RS256)
    {
        return Err(invalid());
    }
    let decoding_key = DecodingKey::from_jwk(key).map_err(|_| invalid())?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[provider.issuer.as_str()]);
    validation.set_audience(&[provider.client_id.as_str()]);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub", "iat"]);
    validation.validate_nbf = true;
    validation.leeway = 30;
    let claims = decode::<Value>(token, &decoding_key, &validation)
        .map_err(|_| invalid())?
        .claims;
    let now = Utc::now().timestamp();
    if claims["nonce"].as_str() != Some(nonce)
        || !claims["sub"].as_str().is_some_and(|s| !s.is_empty() && s.len() <= 255)
        || !claims["iat"].as_i64().is_some_and(|t| t <= now + 30 && t >= now - 330)
        || !claims["auth_time"]
            .as_i64()
            .is_some_and(|t| t <= now + 30 && t >= now - 330)
        || (claims["aud"].as_array().is_some_and(|a| a.len() > 1)
            && claims["azp"].as_str() != Some(provider.client_id.as_str()))
        || (claims.get("azp").is_some() && claims["azp"].as_str() != Some(provider.client_id.as_str()))
    {
        return Err(invalid());
    }
    Ok(claims)
}

async fn exchange(state: &AppState, flow: &Value, code: &str) -> Result<Value, ApiError> {
    if code.is_empty() || code.len() > 4096 {
        return Err(ApiError::bad_request("Invalid authorization response."));
    }
    let provider = enabled_provider(state).await?;
    if serde_json::to_value(&provider).map_err(ApiError::internal)? != flow["provider"] {
        return Err(ApiError::bad_request("Provider settings changed. Start sign-in again."));
    }
    let secret = state
        .storage
        .identity_secret()
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let url = https_url(&provider.token_endpoint)?;
    let response = http_client(&url)
        .await?
        .post(url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", flow["callback"].as_str().unwrap_or_default()),
            ("client_id", provider.client_id.as_str()),
            ("client_secret", secret.as_str()),
            ("code_verifier", flow["verifier"].as_str().unwrap_or_default()),
        ])
        .send()
        .await
        .map_err(|_| ApiError::bad_request("Provider token exchange failed. Start sign-in again."))?;
    let response = bounded_json(response).await?;
    let token = response["id_token"]
        .as_str()
        .ok_or_else(|| ApiError::bad_request("The provider returned no identity token."))?;
    let keys = fetch_json(&provider.jwks_uri).await?;
    let claims = token_claims(token, &keys, &provider, flow["nonce"].as_str().unwrap_or_default())?;
    let user_id = resolve_identity(state, &provider, &claims, &flow["context"]).await?;
    let version = state
        .storage
        .user_session_version(&user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    Ok(json!({"user_id":user_id,"version":version,"provider":provider,"claims":claims}))
}

async fn resolve_identity(
    state: &AppState,
    provider: &IdentityProvider,
    claims: &Value,
    context: &Value,
) -> Result<String, ApiError> {
    let subject = claims["sub"].as_str().ok_or_else(ApiError::unauthorized)?;
    let linked = state
        .storage
        .linked_identity_user(&provider.issuer, subject)
        .await
        .map_err(ApiError::internal)?;
    if let Some(user) = context["link_user"].as_str() {
        let version = context["link_version"].as_i64().ok_or_else(ApiError::unauthorized)?;
        if linked.as_deref() == Some(user) {
            if state
                .storage
                .user_access(user)
                .await
                .map_err(ApiError::internal)?
                .is_none_or(|a| a.session_version != version)
            {
                return Err(ApiError::unauthorized());
            }
            return Ok(user.into());
        }
        state
            .storage
            .link_identity(&provider.issuer, subject, user, version)
            .await
            .map_err(map_storage_error)?;
        return Ok(user.into());
    }
    if let Some(user) = linked {
        return Ok(user);
    }
    let email = claims["email"]
        .as_str()
        .filter(|_| claims["email_verified"] == true)
        .ok_or_else(|| ApiError::bad_request("New enrollment requires a verified provider email and an invitation."))?
        .trim()
        .to_lowercase();
    if state
        .storage
        .find_user_by_email(&email)
        .await
        .map_err(ApiError::internal)?
        .is_some()
    {
        return Err(ApiError::conflict(
            "An account already uses this email. Sign in locally and explicitly link the provider.",
        ));
    }
    if !provider.allowed_domains.is_empty()
        && !email
            .rsplit_once('@')
            .is_some_and(|(_, d)| provider.allowed_domains.iter().any(|allowed| allowed == d))
    {
        return Err(ApiError::forbidden_because(
            "This email domain is not allowed for provider enrollment.",
        ));
    }
    let invitation = context["invitation_token"].as_str().ok_or_else(|| {
        ApiError::bad_request("Ask an administrator for an invitation before enrolling this identity.")
    })?;
    access::invitation_allows_registration(state, invitation, &email).await?;
    let password_hash = auth::hash_password(&random_secret()?).await?;
    let name = claims["name"]
        .as_str()
        .unwrap_or(&email)
        .chars()
        .take(200)
        .collect::<String>();
    let user = state
        .storage
        .create_identity_user(&provider.issuer, subject, &email, &name, &password_hash)
        .await
        .map_err(map_storage_error)?;
    let subject = AuthenticatedSubject {
        user_id: user.id.clone(),
        expires_at: (Utc::now().timestamp() + 300) as u64,
        session_version: 0,
        is_system_admin: false,
        is_app_admin: false,
        token: None,
    };
    let token = serde_json::from_value(json!({"token":invitation})).map_err(ApiError::internal)?;
    let _ = access::accept_invitation(SessionSubject(subject), State(state.clone()), Json(token)).await?;
    state
        .storage
        .confirm_identity_enrollment(&user.id)
        .await
        .map_err(ApiError::internal)?;
    state
        .storage
        .mark_email_verified(&user.id)
        .await
        .map_err(ApiError::internal)?;
    Ok(user.id)
}

pub(crate) async fn callback(
    State(state): State<AppState>,
    client: ClientIp,
    Query(query): Query<Callback>,
) -> Result<Response, ApiError> {
    auth::check_auth_quota(&state, &client, auth::AuthAction::SignIn)?;
    if query.state.len() > 128 {
        return Err(ApiError::bad_request("Invalid sign-in state."));
    }
    let flow = state
        .storage
        .take_identity_callback(&query.state)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| {
            ApiError::bad_request("This sign-in expired or was already used. Return to RepoMemo and start again.")
        })?;
    let result = if query.error.is_some() {
        Err(ApiError::bad_request(
            "Provider sign-in was cancelled or refused. Try again or use local sign-in.",
        ))
    } else {
        exchange(&state, &flow, query.code.as_deref().unwrap_or_default()).await
    };
    let success = result.is_ok();
    let result = result.unwrap_or_else(|error| json!({"error":error.message}));
    state
        .storage
        .finish_identity_flow(&query.state, &result, success)
        .await
        .map_err(ApiError::internal)?;
    Ok((
        [
            (axum::http::header::CACHE_CONTROL, "no-store"),
            (axum::http::header::REFERRER_POLICY, "no-referrer"),
        ],
        "Return to RepoMemo to finish signing in. You can close this page.",
    )
        .into_response())
}

#[derive(Deserialize)]
pub(crate) struct Complete {
    state: String,
    proof: String,
}

pub(crate) async fn complete(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<Complete>,
) -> Result<Response, ApiError> {
    auth::check_auth_quota(&state, &client, auth::AuthAction::Refresh)?;
    if request.state.len() > 128 || request.proof.len() > 128 {
        return Err(ApiError::unauthorized());
    }
    let (phase, result) = state
        .storage
        .complete_identity_flow(&request.state, &request.proof)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::bad_request("This sign-in expired or was already completed. Start again."))?;
    if phase == "pending" {
        return Ok((StatusCode::ACCEPTED, Json(json!({"status":"pending"}))).into_response());
    }
    if phase == "failed" {
        return Err(ApiError::bad_request(
            result["error"].as_str().unwrap_or("Sign-in failed. Try again."),
        ));
    }
    let provider = enabled_provider(&state).await?;
    if serde_json::to_value(&provider).map_err(ApiError::internal)? != result["provider"] {
        return Err(ApiError::bad_request("Provider settings changed. Start again."));
    }
    let user_id = result["user_id"].as_str().ok_or_else(ApiError::unauthorized)?;
    if state
        .storage
        .user_access(user_id)
        .await
        .map_err(ApiError::internal)?
        .is_none_or(|a| Some(a.session_version) != result["version"].as_i64())
    {
        return Err(ApiError::unauthorized());
    }
    let user = state
        .storage
        .find_user(user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    state
        .storage
        .touch_user_connection(user_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(auth::issue_session(&state, user).await?).into_response())
}

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
    #[serde(default)]
    enforced: bool,
    #[serde(default)]
    break_glass_emails: Vec<String>,
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
        enforced: request.enforced,
        ..Default::default()
    })
}

pub(crate) async fn get_provider(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    system::require_full_system_admin(&subject)?;
    let provider = state.storage.identity_provider().await.map_err(ApiError::internal)?;
    provider_overview(&state, provider).await
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
    if provider.enforced {
        provider.break_glass =
            enforcement_ready(&state, &subject.user_id, &provider, &request.break_glass_emails).await?;
    }
    let was_enforced = old.as_ref().is_some_and(|p| p.enforced);
    state
        .storage
        .save_identity_provider(&provider, &secret, &subject.user_id)
        .await
        .map_err(ApiError::internal)?;
    if was_enforced != provider.enforced {
        state
            .storage
            .record_system_event(
                Some(&subject.user_id),
                "identity_enforcement_changed",
                if provider.enforced {
                    "Provider sign-in is now required; password sign-in is limited to break-glass administrators"
                } else {
                    "Provider sign-in is no longer required"
                },
            )
            .await
            .map_err(ApiError::internal)?;
    }
    provider_overview(&state, Some(provider)).await
}

async fn provider_overview(state: &AppState, provider: Option<IdentityProvider>) -> Result<Json<Value>, ApiError> {
    let mut emails = Vec::new();
    for id in provider.iter().flat_map(|p| p.break_glass.iter()) {
        if let Some(email) = state
            .storage
            .find_user(id)
            .await
            .map_err(ApiError::internal)?
            .and_then(|u| u.email)
        {
            emails.push(email);
        }
    }
    Ok(Json(
        json!({"provider":provider,"break_glass_emails":emails,"has_secret":state.storage.identity_secret().await.map_err(ApiError::internal)?.is_some(),"callback_url":state.mail.link("/v1/auth/identity/callback")}),
    ))
}

/// Enforcement may only be saved when the provider is enabled and checked, the administrator saving it has already
/// linked their own account to it, and at least one active system administrator keeps password access.
async fn enforcement_ready(
    state: &AppState,
    admin: &str,
    provider: &IdentityProvider,
    emails: &[String],
) -> Result<Vec<String>, ApiError> {
    if !provider.enabled {
        return Err(ApiError::bad_request(
            "Enable and check the provider before requiring it.",
        ));
    }
    let linked = state.storage.identity_links(admin).await.map_err(ApiError::internal)?;
    if !linked.iter().any(|(issuer, _)| *issuer == provider.issuer) {
        return Err(ApiError::bad_request(
            "Link your own account to this provider on your profile before requiring provider sign-in.",
        ));
    }
    if emails.is_empty() || emails.len() > 5 {
        return Err(ApiError::bad_request(
            "Name one to five system administrators who keep password sign-in as break-glass access.",
        ));
    }
    let mut ids = Vec::new();
    for email in emails {
        let user = state
            .storage
            .find_user_by_email(email)
            .await
            .map_err(|_| ApiError::bad_request("Enter valid break-glass email addresses."))?;
        let active_admin = match &user {
            Some(user) => state
                .storage
                .user_access(&user.id)
                .await
                .map_err(ApiError::internal)?
                .is_some_and(|a| a.is_system_admin),
            None => false,
        };
        let Some(user) = user.filter(|_| active_admin) else {
            return Err(ApiError::bad_request(format!(
                "{email} is not an active system administrator."
            )));
        };
        if !ids.contains(&user.id) {
            ids.push(user.id);
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_tokens_require_signature_issuer_audience_nonce_and_freshness() {
        let provider = IdentityProvider {
            issuer: "https://id.example.com".into(),
            client_id: "memo".into(),
            ..Default::default()
        };
        let now = Utc::now().timestamp();
        let claims = json!({"sub":"subject","iss":provider.issuer,"aud":"memo","iat":now,"auth_time":now,"exp":now+300,"nonce":"nonce"});
        let keys = json!({"keys":[{"kty":"RSA","kid":"test","alg":"RS256","use":"sig","e":"AQAB","n":"yRE6rHuNR0QbHO3H3Kt2pOKGVhQqGZXInOduQNxXzuKlvQTLUTv4l4sggh5_CYYi_cvI-SXVT9kPWSKXxJXBXd_4LkvcPuUakBoAkfh-eiFVMh2VrUyWyj3MFl0HTVF9KwRXLAcwkREiS3npThHRyIxuy0ZMeZfxVL5arMhw1SRELB8HoGfG_AtH89BIE9jDBHZ9dLelK9a184zAf8LwoPLxvJb3Il5nncqPcSfKDDodMFBIMc4lQzDKL5gvmiXLXB1AGLm8KBjfE8s3L5xqi-yUod-j8MtvIj812dkS4QMiRVN_by2h3ZY8LYVGrqZXZTcgn2ujn8uKjXLZVD5TdQ"}]});
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test".into());
        let sign = |c: &Value| {
            encode(
                &header,
                c,
                &EncodingKey::from_rsa_der(include_bytes!("tests/fixtures/oidc-test-key.der")),
            )
            .unwrap()
        };
        assert!(token_claims(&sign(&claims), &keys, &provider, "nonce").is_ok());
        for (field, value) in [
            ("iss", json!("https://other.example.com")),
            ("aud", json!("other")),
            ("nonce", json!("other")),
            ("exp", json!(now - 60)),
            ("iat", json!(now + 60)),
            ("auth_time", json!(now - 600)),
            ("azp", json!("other")),
        ] {
            let mut invalid = claims.clone();
            invalid[field] = value;
            assert!(
                token_claims(&sign(&invalid), &keys, &provider, "nonce").is_err(),
                "{field}"
            );
        }
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(b"wrong-key"),
        )
        .unwrap();
        assert!(token_claims(&token, &keys, &provider, "nonce").is_err());
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
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
