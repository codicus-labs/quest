//! `POST /v1/ai/capture` against a local OpenAI-compatible mock (never a real provider) and a fake
//! provider.

mod common;
#[path = "ai/mock.rs"]
mod mock;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use common::TestServer;
use futures_util::future::BoxFuture;
use mock::{MockProvider, Reply};
use quest_domain::EntityId;
use quest_protocol::{
    AI_CAPTURE_PATH, ApiError, CAPTURE_VERSION, CaptureContext, CaptureRequest, CaptureResponse, ContextQuest,
    ErrorCode, MAX_CAPTURE_TEXT_CHARS, QuestRef, WorkTarget,
};
use quest_server::{
    AiConfig, Completion, LlmProvider, OpenAiCompatible, OpenAiSettings, OpenRouterOptions, ProviderError, RateLimits,
    ReasoningEffort, StructuredRequest,
};
use serde_json::{Value, json};
use uuid::Uuid;

const TEXT: &str = "Morgen Angebot für Müller schreiben, dauert etwa zwei Stunden, gehört zu Website-Relaunch. \
Neue Quest Steuererklärung mit Belege sammeln am Freitag. Heute von neun bis elf am Relaunch-Konzept gearbeitet.";

fn relaunch() -> EntityId {
    EntityId(Uuid::from_u128(0x1234))
}

fn capture_request(text: &str) -> CaptureRequest {
    CaptureRequest {
        version: CAPTURE_VERSION,
        text: text.to_owned(),
        // Tuesday, 11:00 in Berlin.
        now: Utc.with_ymd_and_hms(2026, 11, 24, 10, 0, 0).unwrap(),
        time_zone: "Europe/Berlin".into(),
        language: "de-DE".into(),
        context: CaptureContext {
            quests: vec![ContextQuest { id: relaunch(), title: "Website-Relaunch".into(), description: String::new() }],
            tasks: vec![],
        },
    }
}

/// What a model would answer for [`TEXT`], shaped like `quest_capture::output_schema`.
fn model_answer() -> Value {
    json!({
        "new_quests": [{ "key": "q1", "title": "Steuererklärung", "description": "", "milestones": [] }],
        "tasks": [
            { "title": "Angebot für Müller schreiben", "description": "", "quest_id": relaunch().0.to_string(),
              "new_quest_key": null, "quest_confidence": "high", "day": "2026-11-25", "start_time": null,
              "duration_minutes": null, "repeat": null },
            { "title": "Belege sammeln", "description": "", "quest_id": null, "new_quest_key": "q1",
              "quest_confidence": "high", "day": "2026-11-27", "start_time": null, "duration_minutes": null,
              "repeat": null },
        ],
        "work_logs": [
            { "task_id": null, "new_task_title": "Relaunch-Konzept", "quest_id": relaunch().0.to_string(),
              "new_quest_key": null, "confidence": "high", "date": "2026-11-24", "start_time": "09:00",
              "end_time": "11:00" },
        ],
    })
}

fn openai(mock: &MockProvider) -> Arc<dyn LlmProvider> {
    provider(mock, None)
}

/// The mock speaking for OpenRouter, with a fallback model and reasoning switched off.
fn openrouter(mock: &MockProvider) -> Arc<dyn LlmProvider> {
    let options = OpenRouterOptions { fallback_models: vec!["mock/fallback".into()], reasoning: ReasoningEffort::None };
    provider(mock, Some(options))
}

fn provider(mock: &MockProvider, openrouter: Option<OpenRouterOptions>) -> Arc<dyn LlmProvider> {
    let settings = OpenAiSettings {
        base_url: mock.base_url(),
        api_key: "test-key".into(),
        model: "mock/model".into(),
        extra_headers: vec![("X-Title".into(), "Quest".into())],
        allow_insecure_http: false,
        openrouter,
    };
    Arc::new(OpenAiCompatible::new(settings).unwrap())
}

const OPENROUTER_FIELDS: [&str; 3] = ["provider", "models", "reasoning"];

struct Setup {
    server: TestServer,
    token: String,
}

impl Setup {
    async fn new(ai: Option<AiConfig>) -> Setup {
        let server = match ai {
            Some(ai) => TestServer::start_with_ai(ai).await,
            None => TestServer::start().await,
        };
        let token = server.sign_in_with_ai("ada").await.token;
        Setup { server, token }
    }

    async fn with_mock(mock: &MockProvider) -> Setup {
        Setup::new(Some(AiConfig::new(openai(mock)))).await
    }

    async fn capture(&self, request: &CaptureRequest) -> Result<CaptureResponse, (u16, ApiError)> {
        let response = self.server.post_json(AI_CAPTURE_PATH, Some(&self.token), request).await;
        let status = response.status().as_u16();
        if status == 200 { Ok(response.json().await.unwrap()) } else { Err((status, response.json().await.unwrap())) }
    }

    async fn capture_error(&self, request: &CaptureRequest) -> (u16, ErrorCode) {
        let (status, error) = self.capture(request).await.unwrap_err();
        (status, error.code)
    }
}

fn assert_example_proposal(response: &CaptureResponse) {
    assert_eq!(response.version, CAPTURE_VERSION);
    let proposal = &response.proposal;
    assert_eq!(proposal.quests.len(), 1);
    assert_eq!(proposal.quests[0].title, "Steuererklärung");
    assert_eq!(proposal.tasks.len(), 2);
    assert_eq!(proposal.tasks[0].quest, Some(QuestRef::Existing { id: relaunch() }));
    assert_eq!(proposal.tasks[0].day, NaiveDate::from_ymd_opt(2026, 11, 25));
    assert_eq!(proposal.tasks[0].block, None, "a duration without a start time is no block");
    assert_eq!(proposal.tasks[1].quest, Some(QuestRef::New { key: proposal.quests[0].key.clone() }));
    assert_eq!(proposal.work.len(), 1);
    let work = &proposal.work[0];
    assert!(matches!(&work.task, WorkTarget::New { title, .. } if title == "Relaunch-Konzept"));
    assert_eq!(
        (work.start, work.end),
        (NaiveTime::from_hms_opt(9, 0, 0).unwrap(), NaiveTime::from_hms_opt(11, 0, 0).unwrap())
    );
    assert!(proposal.dropped.is_empty(), "{:?}", proposal.dropped);
}

#[tokio::test]
async fn capture_asks_for_strict_json_schema_and_returns_the_validated_proposal() {
    let mock = MockProvider::start(vec![Reply::content(&model_answer().to_string())]).await;
    let setup = Setup::with_mock(&mock).await;

    let response = setup.capture(&capture_request(TEXT)).await.unwrap();

    assert_example_proposal(&response);
    let recorded = mock.recorded();
    assert_eq!(recorded.len(), 1);
    let sent = &recorded[0];
    assert_eq!(sent.headers["authorization"], "Bearer test-key");
    assert_eq!(sent.headers["x-title"], "Quest");
    assert_eq!(sent.body["model"], "mock/model");
    assert_eq!(sent.body["response_format"]["type"], "json_schema");
    assert_eq!(sent.body["response_format"]["json_schema"]["name"], quest_capture::SCHEMA_NAME);
    assert_eq!(sent.body["response_format"]["json_schema"]["strict"], true);
    assert_eq!(sent.body["response_format"]["json_schema"]["schema"], quest_capture::output_schema());
    assert_eq!(sent.body["messages"][0]["role"], "system");
    assert!(sent.body["messages"][1]["content"].as_str().unwrap().contains("Steuererklärung"));
    assert!(sent.body.get("tools").is_none());
    for field in OPENROUTER_FIELDS {
        assert!(sent.body.get(field).is_none(), "{field} is only for OpenRouter");
    }
}

#[tokio::test]
async fn openrouter_requires_all_parameters_and_gets_fallbacks_and_reasoning() {
    let mock = MockProvider::start(vec![Reply::content(&model_answer().to_string())]).await;
    let setup = Setup::new(Some(AiConfig::new(openrouter(&mock)))).await;

    assert_example_proposal(&setup.capture(&capture_request(TEXT)).await.unwrap());

    let sent = &mock.recorded()[0].body;
    assert_eq!(sent["model"], "mock/model");
    assert_eq!(sent["provider"], json!({ "require_parameters": true }));
    assert_eq!(sent["models"], json!(["mock/model", "mock/fallback"]));
    assert_eq!(sent["reasoning"], json!({ "effort": "none" }));
    assert_eq!(sent["response_format"]["json_schema"]["strict"], true);
}

#[tokio::test]
async fn openrouter_without_fallbacks_or_reasoning_setting_sends_neither() {
    let mock = MockProvider::start(vec![Reply::content(&model_answer().to_string())]).await;
    let options = OpenRouterOptions { fallback_models: vec![], reasoning: ReasoningEffort::ModelDefault };
    let setup = Setup::new(Some(AiConfig::new(provider(&mock, Some(options))))).await;

    setup.capture(&capture_request(TEXT)).await.unwrap();

    let sent = &mock.recorded()[0].body;
    assert_eq!(sent["provider"], json!({ "require_parameters": true }));
    assert!(sent.get("models").is_none());
    assert!(sent.get("reasoning").is_none());
}

#[tokio::test]
async fn openrouter_without_a_json_schema_provider_falls_back_to_tool_calling() {
    let name = quest_capture::SCHEMA_NAME;
    let mock = MockProvider::start(vec![
        Reply::no_endpoints_for_parameters(),
        Reply::tool_call(name, &model_answer()),
        Reply::tool_call(name, &model_answer()),
    ])
    .await;
    let setup = Setup::new(Some(AiConfig::new(openrouter(&mock)))).await;

    assert_example_proposal(&setup.capture(&capture_request(TEXT)).await.unwrap());
    assert_example_proposal(&setup.capture(&capture_request(TEXT)).await.unwrap());

    let recorded = mock.recorded();
    assert_eq!(recorded.len(), 3, "one fallback, then tool calling right away");
    for sent in &recorded[1..] {
        assert!(sent.body.get("response_format").is_none());
        assert_eq!(sent.body["tool_choice"]["function"]["name"], name);
        assert_eq!(sent.body["provider"], json!({ "require_parameters": true }), "tools must be honoured too");
    }
}

#[tokio::test]
async fn a_404_from_other_apis_does_not_fall_back() {
    let mock = MockProvider::start(vec![Reply::no_endpoints_for_parameters()]).await;
    let setup = Setup::with_mock(&mock).await;
    assert_eq!(setup.capture_error(&capture_request(TEXT)).await, (502, ErrorCode::AiUnavailable));
    assert_eq!(mock.recorded().len(), 1);
}

#[tokio::test]
async fn fenced_content_is_accepted() {
    let fenced = format!("```json\n{}\n```", model_answer());
    let mock = MockProvider::start(vec![Reply::content(&fenced)]).await;
    let setup = Setup::with_mock(&mock).await;
    assert_example_proposal(&setup.capture(&capture_request(TEXT)).await.unwrap());
}

#[tokio::test]
async fn rejected_json_schema_falls_back_to_tool_calling_and_stays_there() {
    let name = quest_capture::SCHEMA_NAME;
    let mock = MockProvider::start(vec![
        Reply::json_schema_unsupported(),
        Reply::tool_call(name, &model_answer()),
        Reply::tool_call(name, &model_answer()),
    ])
    .await;
    let setup = Setup::with_mock(&mock).await;

    assert_example_proposal(&setup.capture(&capture_request(TEXT)).await.unwrap());
    assert_example_proposal(&setup.capture(&capture_request(TEXT)).await.unwrap());

    let recorded = mock.recorded();
    assert_eq!(recorded.len(), 3, "one fallback, then tool calling right away");
    assert!(recorded[0].body.get("response_format").is_some());
    for sent in &recorded[1..] {
        assert!(sent.body.get("response_format").is_none());
        assert_eq!(sent.body["tools"][0]["function"]["name"], name);
        assert_eq!(sent.body["tools"][0]["function"]["parameters"], quest_capture::output_schema());
        assert_eq!(sent.body["tool_choice"]["function"]["name"], name);
    }
}

#[tokio::test]
async fn other_bad_requests_do_not_fall_back() {
    let reply = Reply::Status(400, json!({ "error": { "message": "Unknown model mock/model" } }));
    let mock = MockProvider::start(vec![reply]).await;
    let setup = Setup::with_mock(&mock).await;
    assert_eq!(setup.capture_error(&capture_request(TEXT)).await, (502, ErrorCode::AiUnavailable));
    assert_eq!(mock.recorded().len(), 1);
}

#[tokio::test]
async fn failing_fallback_reports_the_tool_call_error_and_is_not_kept() {
    let rate_limited = Reply::Status(429, json!({ "error": { "message": "Rate limit reached" } }));
    let answer = Reply::content(&model_answer().to_string());
    let mock = MockProvider::start(vec![Reply::json_schema_unsupported(), rate_limited, answer]).await;
    let setup = Setup::with_mock(&mock).await;
    assert_eq!(setup.capture_error(&capture_request(TEXT)).await, (503, ErrorCode::AiUnavailable));
    assert_example_proposal(&setup.capture(&capture_request(TEXT)).await.unwrap());
    let recorded = mock.recorded();
    assert_eq!(recorded.len(), 3);
    assert!(recorded[2].body.get("response_format").is_some(), "json_schema again after a failed fallback");
}

#[tokio::test]
async fn tool_call_without_json_arguments_is_ai_invalid_response() {
    let mock = MockProvider::start(vec![
        Reply::json_schema_unsupported(),
        Reply::tool_call(quest_capture::SCHEMA_NAME, &json!("not json")),
    ])
    .await;
    let setup = Setup::with_mock(&mock).await;
    assert_eq!(setup.capture_error(&capture_request(TEXT)).await, (502, ErrorCode::AiInvalidResponse));
}

#[tokio::test]
async fn slow_provider_times_out_as_ai_unavailable() {
    let mock = MockProvider::start(vec![Reply::Slow(Duration::from_secs(5))]).await;
    let mut ai = AiConfig::new(openai(&mock));
    ai.timeout = Duration::from_millis(200);
    let setup = Setup::new(Some(ai)).await;
    assert_eq!(setup.capture_error(&capture_request(TEXT)).await, (504, ErrorCode::AiUnavailable));
}

#[tokio::test]
async fn provider_rate_limit_is_ai_unavailable() {
    let reply = Reply::Status(429, json!({ "error": { "message": "Rate limit reached" } }));
    let mock = MockProvider::start(vec![reply]).await;
    let setup = Setup::with_mock(&mock).await;
    assert_eq!(setup.capture_error(&capture_request(TEXT)).await, (503, ErrorCode::AiUnavailable));
}

#[tokio::test]
async fn provider_errors_are_ai_unavailable() {
    let reply = Reply::Status(401, json!({ "error": { "message": "Invalid API key" } }));
    let mock = MockProvider::start(vec![reply]).await;
    let setup = Setup::with_mock(&mock).await;
    assert_eq!(setup.capture_error(&capture_request(TEXT)).await, (502, ErrorCode::AiUnavailable));
}

#[tokio::test]
async fn unreachable_provider_is_ai_unavailable() {
    let mock = MockProvider::start(vec![]).await;
    let provider = openai(&mock);
    drop(mock);
    let setup = Setup::new(Some(AiConfig::new(provider))).await;
    assert_eq!(setup.capture_error(&capture_request(TEXT)).await, (502, ErrorCode::AiUnavailable));
}

#[tokio::test]
async fn unusable_answers_are_ai_invalid_response() {
    for content in ["this is not json", "[]", "{\"something\": []}"] {
        let mock = MockProvider::start(vec![Reply::content(content)]).await;
        let setup = Setup::with_mock(&mock).await;
        assert_eq!(setup.capture_error(&capture_request(TEXT)).await, (502, ErrorCode::AiInvalidResponse), "{content}");
    }
}

#[tokio::test]
async fn without_configuration_capture_is_ai_not_configured() {
    let setup = Setup::new(None).await;
    assert_eq!(setup.capture_error(&capture_request(TEXT)).await, (503, ErrorCode::AiNotConfigured));
}

#[tokio::test]
async fn capture_needs_the_accounts_ai_opt_in() {
    let provider = Arc::new(FakeProvider::default());
    let setup = Setup::new(Some(AiConfig::new(provider.clone()))).await;
    let server = &setup.server;
    let other = server.sign_in("grace").await.token;
    let account = server.account(&other).await.unwrap();
    assert!(!account.ai_enabled, "off until the user turns it on");
    assert!(account.ai_available);
    let response = server.post_json(AI_CAPTURE_PATH, Some(&other), &capture_request(TEXT)).await;
    assert_eq!(response.status().as_u16(), 403);
    assert_eq!(response.json::<ApiError>().await.unwrap().code, ErrorCode::AiDisabled);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0, "nothing is sent to the provider");

    assert!(server.set_ai_enabled(&other, true).await.ai_enabled);
    let response = server.post_json(AI_CAPTURE_PATH, Some(&other), &capture_request(TEXT)).await;
    assert_eq!(response.status().as_u16(), 200);
    assert!(!server.set_ai_enabled(&other, false).await.ai_enabled);
    let response = server.post_json(AI_CAPTURE_PATH, Some(&other), &capture_request(TEXT)).await;
    assert_eq!(response.status().as_u16(), 403, "turning it off takes effect at once");
}

#[tokio::test]
async fn capture_requires_a_valid_bearer_token() {
    let mock = MockProvider::start(vec![]).await;
    let setup = Setup::with_mock(&mock).await;
    for token in [None, Some("not-a-token")] {
        let response = setup.server.post_json(AI_CAPTURE_PATH, token, &capture_request(TEXT)).await;
        assert_eq!(response.status().as_u16(), 401);
        assert_eq!(response.json::<ApiError>().await.unwrap().code, ErrorCode::Unauthorized);
    }
    assert!(mock.recorded().is_empty());
}

#[tokio::test]
async fn too_long_text_and_oversized_bodies_are_refused_before_the_provider() {
    let mock = MockProvider::start(vec![]).await;
    let setup = Setup::with_mock(&mock).await;
    let too_long = "a".repeat(MAX_CAPTURE_TEXT_CHARS + 1);
    assert_eq!(setup.capture_error(&capture_request(&too_long)).await, (413, ErrorCode::TextTooLong));
    let huge = "a".repeat(600 * 1024);
    assert_eq!(setup.capture_error(&capture_request(&huge)).await, (413, ErrorCode::TextTooLong));
    assert!(mock.recorded().is_empty());
}

#[tokio::test]
async fn malformed_requests_are_invalid_request() {
    let mock = MockProvider::start(vec![]).await;
    let setup = Setup::with_mock(&mock).await;
    let mut bad_zone = capture_request(TEXT);
    bad_zone.time_zone = "Mars/Olympus".into();
    assert_eq!(setup.capture_error(&bad_zone).await, (400, ErrorCode::InvalidRequest));
    let response = setup.server.post_json(AI_CAPTURE_PATH, Some(&setup.token), &json!({ "text": "hi" })).await;
    assert_eq!(response.status().as_u16(), 400);
    assert_eq!(response.json::<ApiError>().await.unwrap().code, ErrorCode::InvalidRequest);
    assert!(mock.recorded().is_empty());
}

/// Answers with [`model_answer`] and counts its calls.
#[derive(Debug, Default)]
struct FakeProvider {
    calls: AtomicUsize,
}

impl LlmProvider for FakeProvider {
    fn complete_structured<'a>(
        &'a self,
        _request: &'a StructuredRequest,
    ) -> BoxFuture<'a, Result<Completion, ProviderError>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(Completion { value: model_answer(), usage: None }) })
    }
}

#[tokio::test]
async fn rate_limit_per_account_refuses_requests_over_the_budget() {
    let provider = Arc::new(FakeProvider::default());
    let mut ai = AiConfig::new(provider.clone());
    ai.limits = RateLimits { per_minute: 2, per_day: 100 };
    let setup = Setup::new(Some(ai)).await;

    assert_example_proposal(&setup.capture(&capture_request(TEXT)).await.unwrap());
    assert_eq!(setup.capture_error(&capture_request("")).await, (400, ErrorCode::InvalidRequest), "counts too");
    assert_eq!(setup.capture_error(&capture_request(TEXT)).await, (429, ErrorCode::RateLimited));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);

    let other = setup.server.sign_in_with_ai("grace").await.token;
    let response = setup.server.post_json(AI_CAPTURE_PATH, Some(&other), &capture_request(TEXT)).await;
    assert_eq!(response.status().as_u16(), 200, "other accounts have their own budget");
}

/// Runs only with `QUEST_AI_BASE_URL`, `QUEST_AI_API_KEY` and `QUEST_AI_MODEL` set:
/// `cargo test -p quest-server --test ai -- --ignored live_provider`. Sends the example text to the
/// configured provider.
#[tokio::test]
#[ignore = "calls a real AI provider; needs QUEST_AI_* and consent to send the example text"]
async fn live_provider() {
    let env = |name| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let base_url = env("QUEST_AI_BASE_URL");
    let openrouter = OpenRouterOptions::for_base_url(&base_url, None, vec![], ReasoningEffort::default()).unwrap();
    let settings = OpenAiSettings {
        base_url,
        api_key: env("QUEST_AI_API_KEY"),
        model: env("QUEST_AI_MODEL"),
        extra_headers: vec![],
        allow_insecure_http: false,
        openrouter,
    };
    let setup = Setup::new(Some(AiConfig::new(Arc::new(OpenAiCompatible::new(settings).unwrap())))).await;
    let response = setup.capture(&capture_request(TEXT)).await.unwrap();
    assert!(!response.proposal.tasks.is_empty());
}
