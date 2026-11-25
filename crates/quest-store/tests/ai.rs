//! AI capture through the store's session against a real, in-process server with a fake provider.

#[allow(dead_code)] // the harness is shared with `sync.rs`, which uses the rest
mod common;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use common::{TestServer, TestStore};
use futures_util::future::BoxFuture;
use quest_protocol::{CAPTURE_VERSION, CaptureContext, CaptureRequest};
use quest_server::{AiConfig, Completion, LlmProvider, ProviderError, StructuredRequest};
use quest_store::CaptureError;
use serde_json::json;

/// Answers every capture with one task for tomorrow.
#[derive(Debug)]
struct FakeProvider;

impl LlmProvider for FakeProvider {
    fn complete_structured<'a>(
        &'a self,
        _request: &'a StructuredRequest,
    ) -> BoxFuture<'a, Result<Completion, ProviderError>> {
        let value = json!({
            "new_quests": [],
            "tasks": [{ "title": "Angebot schreiben", "description": "", "quest_id": null, "new_quest_key": null,
                        "quest_confidence": "high", "planned_on": "2026-11-25", "due_on": "2026-11-27",
                        "repeat": null }],
            "work_logs": [],
        });
        Box::pin(async move { Ok(Completion { value, usage: None }) })
    }
}

fn request(text: &str) -> CaptureRequest {
    let now: DateTime<Utc> = "2026-11-24T09:30:00Z".parse().unwrap();
    CaptureRequest {
        version: CAPTURE_VERSION,
        text: text.into(),
        now,
        time_zone: "Europe/Berlin".into(),
        language: "de-DE".into(),
        context: CaptureContext { quests: vec![], tasks: vec![] },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_needs_an_account_and_the_ai_opt_in() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::with_ai(dir.path(), AiConfig::new(Arc::new(FakeProvider)));
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    let text = "Morgen um 9 zwei Stunden Angebot schreiben";
    let ai = a.store().ai();
    let signed_out = tokio::task::spawn_blocking(move || ai.capture(&request(text)));
    assert_eq!(signed_out.await.unwrap(), Err(CaptureError::SignedOut));

    a.sign_in(&server, "anna").await.unwrap();
    assert!(a.store().account().unwrap().info.ai_available);
    let ai = a.store().ai();
    let disabled = tokio::task::spawn_blocking(move || ai.capture(&request(text)));
    assert_eq!(disabled.await.unwrap(), Err(CaptureError::Disabled));

    a.blocking(|_, store| (None, store.set_ai_enabled(true))).await.unwrap();
    let ai = a.store().ai();
    let proposal =
        tokio::task::spawn_blocking(move || ai.capture(&request(text))).await.unwrap().expect("the server proposes");
    assert_eq!(proposal.tasks.len(), 1);
    assert_eq!(proposal.tasks[0].title, "Angebot schreiben");
    assert_eq!(proposal.tasks[0].due_on, chrono::NaiveDate::from_ymd_opt(2026, 11, 27));
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_server_without_ai_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    a.sign_in(&server, "anna").await.unwrap();
    assert!(!a.store().account().unwrap().info.ai_available);
    a.blocking(|_, store| (None, store.set_ai_enabled(true))).await.unwrap();
    let ai = a.store().ai();
    let result = tokio::task::spawn_blocking(move || ai.capture(&request("Morgen Angebot schreiben"))).await.unwrap();
    assert_eq!(result, Err(CaptureError::NotConfigured));
    server.stop().await;
}
