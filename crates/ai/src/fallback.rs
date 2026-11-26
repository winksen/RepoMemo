//! A model with a fallback (V0.9.80): when the model times out, answers with a server error or is rate limited, the fallback answers
//! instead, once for the run and from then on. A refusal that would repeat (a rejected key, a wrong model name) is not retried:
//! another model would not fix it, and hiding it would leave the administrator without the message. Only chat is covered, which is
//! what CAIRN's loop uses; every other call goes to the model itself.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use anyhow::Result;
use repomemo_domain::ProviderTestResult;
use serde_json::Value;

use crate::{AiProvider, ChatRequest, ChatResponse, GenerateRequest, ImageAnalysisRequest, ProviderHttpError};

/// Whether an error is the kind a second model can answer: the request timed out, or the server answered `5xx` or `429`.
pub fn is_fallback_error(error: &anyhow::Error) -> bool {
    if let Some(http) = error.downcast_ref::<ProviderHttpError>() {
        return http.status == 429 || (500..=599).contains(&http.status);
    }
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<reqwest::Error>())
        .any(reqwest::Error::is_timeout)
}

/// What made the fallback answer, in words for a trace.
fn reason_of(error: &anyhow::Error) -> String {
    match error.downcast_ref::<ProviderHttpError>() {
        Some(http) if http.status == 429 => "was rate limited".to_owned(),
        Some(http) => format!("answered with a server error (HTTP {})", http.status),
        None => "did not answer in time".to_owned(),
    }
}

/// A provider that falls back to a second one. `P` is the model, `F` the fallback.
pub struct FallbackProvider<P, F> {
    primary: P,
    fallback: Option<F>,
    /// Set once the fallback answered: the rest of the run goes to it, so a model that is down is not waited for at every step.
    switched: AtomicBool,
    reason: Mutex<Option<String>>,
}

impl<P: AiProvider, F: AiProvider> FallbackProvider<P, F> {
    pub fn new(primary: P, fallback: Option<F>) -> Self {
        Self {
            primary,
            fallback,
            switched: AtomicBool::new(false),
            reason: Mutex::new(None),
        }
    }

    /// Whether the fallback answered.
    pub fn used_fallback(&self) -> bool {
        self.switched.load(Ordering::SeqCst)
    }

    /// Why the model did not answer, when the fallback did: `timed out`, `was rate limited` or `answered with a server error`.
    pub fn fallback_reason(&self) -> Option<String> {
        self.reason.lock().ok().and_then(|reason| reason.clone())
    }
}

impl<P: AiProvider, F: AiProvider> AiProvider for FallbackProvider<P, F> {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let Some(fallback) = &self.fallback else {
            return self.primary.chat(request).await;
        };
        if self.switched.load(Ordering::SeqCst) {
            return fallback.chat(request).await;
        }
        match self.primary.chat(request.clone()).await {
            Err(error) if is_fallback_error(&error) => {
                tracing::warn!(error = %error, "A model did not answer; its fallback answers instead");
                let answered = fallback.chat(request).await?;
                self.switched.store(true, Ordering::SeqCst);
                if let Ok(mut reason) = self.reason.lock() {
                    *reason = Some(reason_of(&error));
                }
                Ok(answered)
            }
            other => other,
        }
    }

    async fn generate(&self, request: GenerateRequest) -> Result<String> {
        self.primary.generate(request).await
    }

    async fn context_tokens(&self) -> Result<Option<u32>> {
        self.primary.context_tokens().await
    }

    async fn embed(&self, texts: Vec<String>, options: Value) -> Result<Vec<Vec<f32>>> {
        self.primary.embed(texts, options).await
    }

    async fn summarize(&self, target: String, options: Value) -> Result<String> {
        self.primary.summarize(target, options).await
    }

    async fn rerank(&self, query: String, candidates: Vec<String>) -> Result<Vec<usize>> {
        self.primary.rerank(query, candidates).await
    }

    async fn analyze_image(&self, request: ImageAnalysisRequest) -> Result<String> {
        self.primary.analyze_image(request).await
    }

    async fn test_connection(&self) -> Result<ProviderTestResult> {
        self.primary.test_connection().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChatMessage, ScriptedProvider};

    fn ask() -> ChatRequest {
        ChatRequest {
            messages: vec![ChatMessage::new(crate::ChatRole::User, "Hello")],
            tools: Vec::new(),
            options: serde_json::json!({}),
        }
    }

    fn http(status: u16) -> anyhow::Error {
        anyhow::Error::new(ProviderHttpError::new(
            status,
            format!("The provider failed (HTTP {status})."),
        ))
    }

    #[test]
    fn only_a_timeout_a_server_error_or_a_rate_limit_is_answered_by_a_fallback() {
        for status in [429, 500, 502, 503, 599] {
            assert!(is_fallback_error(&http(status)), "{status}");
        }
        for status in [400, 401, 402, 403, 404, 422] {
            assert!(!is_fallback_error(&http(status)), "{status}");
        }
        assert!(!is_fallback_error(&anyhow::anyhow!("an invalid reply")));
    }

    #[tokio::test]
    async fn a_failing_model_falls_back_once_and_the_fallback_answers_the_rest_of_the_run() {
        let primary = ScriptedProvider::with_results(vec![Err(http(503))]);
        let fallback = ScriptedProvider::new(vec![
            ChatResponse::text("From the fallback."),
            ChatResponse::text("Again."),
        ]);
        let provider = FallbackProvider::new(primary, Some(fallback));
        assert!(!provider.used_fallback());
        assert_eq!(provider.chat(ask()).await.unwrap().content, "From the fallback.");
        assert!(provider.used_fallback());
        assert_eq!(
            provider.fallback_reason().as_deref(),
            Some("answered with a server error (HTTP 503)")
        );
        // The model that is down is not tried again for the same run.
        assert_eq!(provider.chat(ask()).await.unwrap().content, "Again.");
        assert_eq!(provider.primary.requests().len(), 1);
        assert_eq!(provider.fallback.as_ref().unwrap().requests().len(), 2);
    }

    #[tokio::test]
    async fn a_refusal_that_would_repeat_is_not_hidden_and_a_working_model_never_needs_the_fallback() {
        let rejected = FallbackProvider::new(
            ScriptedProvider::with_results(vec![Err(http(401))]),
            Some(ScriptedProvider::new(vec![ChatResponse::text("never asked")])),
        );
        assert!(rejected.chat(ask()).await.is_err());
        assert!(!rejected.used_fallback());
        assert!(rejected.fallback.as_ref().unwrap().requests().is_empty());

        let working = FallbackProvider::new(
            ScriptedProvider::new(vec![ChatResponse::text("Fine.")]),
            Some(ScriptedProvider::new(Vec::new())),
        );
        assert_eq!(working.chat(ask()).await.unwrap().content, "Fine.");
        assert!(!working.used_fallback());

        // Without a fallback the error is the model's own; a failing fallback is the error of the request.
        let alone =
            FallbackProvider::<_, ScriptedProvider>::new(ScriptedProvider::with_results(vec![Err(http(503))]), None);
        assert!(alone.chat(ask()).await.is_err());
        let both = FallbackProvider::new(
            ScriptedProvider::with_results(vec![Err(http(429))]),
            Some(ScriptedProvider::with_results(vec![Err(http(500))])),
        );
        assert!(both.chat(ask()).await.is_err());
        assert!(!both.used_fallback());
    }
}
