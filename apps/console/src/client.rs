//! HTTP access to a RepoMemo server: sign-in, silent token renewal, and the
//! two console endpoints the web page uses too.

use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};

/// Why a request did not produce an answer.
#[derive(Debug)]
pub enum Failure {
    /// The server refused it; the message says why (usage, rights, wrong password).
    Refused(String),
    /// The session ended and could not be renewed.
    Expired,
    /// The server could not be reached.
    Unreachable(String),
    /// The server failed while answering.
    Server(String),
}

/// One command of the server's catalog.
#[derive(Debug, Clone, Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub usage: String,
}

/// What the server greets a console with (`GET /v1/system/console`).
#[derive(Debug, Deserialize)]
pub struct Welcome {
    pub tips: String,
    pub version: String,
    pub user: String,
    pub role: String,
    pub commands: Vec<CommandInfo>,
}

#[derive(Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: String,
}

pub struct Client {
    http: reqwest::Client,
    base: String,
    access: Option<String>,
    refresh: Option<String>,
}

impl Client {
    /// `token` is an access token from `REPOMEMO_TOKEN`, used instead of signing in.
    pub fn new(base: &str, token: Option<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: base.trim_end_matches('/').to_owned(),
            access: token,
            refresh: None,
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn is_signed_in(&self) -> bool {
        self.access.is_some()
    }

    pub async fn sign_in(&mut self, email: &str, password: &str) -> Result<(), Failure> {
        let answer = self
            .call(Method::POST, "/v1/auth/login", Some(json!({ "email": email, "password": password })), false)
            .await?;
        self.store(answer)
    }

    pub async fn welcome(&mut self) -> Result<Welcome, Failure> {
        let answer = self.authorized(Method::GET, "/v1/system/console", None).await?;
        serde_json::from_value(answer).map_err(|error| Failure::Server(format!("The server's welcome was not understood: {error}")))
    }

    /// Runs one console line; the answer is the server's JSON as is.
    pub async fn run(&mut self, command: &str) -> Result<Value, Failure> {
        self.authorized(Method::POST, "/v1/system/console", Some(json!({ "command": command }))).await
    }

    fn store(&mut self, answer: Value) -> Result<(), Failure> {
        let tokens: Tokens = serde_json::from_value(answer)
            .map_err(|error| Failure::Server(format!("The sign-in answer was not understood: {error}")))?;
        self.access = Some(tokens.access_token);
        self.refresh = Some(tokens.refresh_token);
        Ok(())
    }

    /// Renews the access token once it has expired, like the web app does.
    async fn renew(&mut self) -> bool {
        let Some(refresh) = self.refresh.clone() else { return false };
        let renewed = match self
            .call(Method::POST, "/v1/auth/refresh", Some(json!({ "refresh_token": refresh })), false)
            .await
        {
            Ok(answer) => self.store(answer).is_ok(),
            Err(_) => false,
        };
        if !renewed {
            self.access = None;
            self.refresh = None;
        }
        renewed
    }

    async fn authorized(&mut self, method: Method, path: &str, body: Option<Value>) -> Result<Value, Failure> {
        let first = self.call(method.clone(), path, body.clone(), true).await;
        if matches!(first, Err(Failure::Expired)) && self.renew().await {
            return self.call(method, path, body, true).await;
        }
        first
    }

    async fn call(&self, method: Method, path: &str, body: Option<Value>, bearer: bool) -> Result<Value, Failure> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base))
            .header("Accept", "application/json")
            // Listed under System › Overview › clients, next to the web app.
            .header("X-RepoMemo-Client", "console");
        if let (true, Some(token)) = (bearer, &self.access) {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(|error| Failure::Unreachable(error.to_string()))?;
        let status = response.status();
        let answer: Value = response.json().await.unwrap_or(Value::Null);
        if status.is_success() {
            return Ok(answer);
        }
        let message = answer["error"]["message"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("The server answered {status}."));
        Err(match status {
            StatusCode::UNAUTHORIZED if bearer => Failure::Expired,
            status if status.is_client_error() => Failure::Refused(message),
            _ => Failure::Server(message),
        })
    }
}
