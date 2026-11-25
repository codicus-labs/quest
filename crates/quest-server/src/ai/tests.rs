use std::time::{Duration, Instant};

use quest_protocol::UserId;
use serde_json::json;
use uuid::Uuid;

use super::openai::{
    AiConfigError, OpenAiCompatible, OpenAiSettings, OpenRouterOptions, ReasoningEffort, chat_completions_url,
    is_openrouter, parse_answer, parse_json_text, rejects_json_schema, status_error,
};
use super::provider::{ProviderError, Usage};
use super::rate_limit::{RateLimiter, RateLimits};

fn user(n: u128) -> UserId {
    UserId(Uuid::from_u128(n))
}

#[test]
fn rate_limiter_allows_the_per_minute_budget_then_refuses_until_a_minute_passed() {
    let mut limiter = RateLimiter::new(RateLimits { per_minute: 2, per_day: 100 });
    let start = Instant::now();
    assert!(limiter.try_acquire(user(1), start));
    assert!(limiter.try_acquire(user(1), start + Duration::from_secs(1)));
    assert!(!limiter.try_acquire(user(1), start + Duration::from_secs(2)));
    assert!(limiter.try_acquire(user(2), start + Duration::from_secs(2)), "other accounts are independent");
    assert!(limiter.try_acquire(user(1), start + Duration::from_secs(61)));
}

#[test]
fn rate_limiter_enforces_the_daily_budget_and_forgets_after_a_day() {
    let mut limiter = RateLimiter::new(RateLimits { per_minute: 10, per_day: 3 });
    let start = Instant::now();
    for minute in 0..3 {
        assert!(limiter.try_acquire(user(1), start + Duration::from_secs(minute * 60)));
    }
    assert!(!limiter.try_acquire(user(1), start + Duration::from_secs(3600)));
    assert!(limiter.try_acquire(user(1), start + Duration::from_secs(24 * 3600)));
}

#[test]
fn refused_requests_do_not_use_up_the_budget() {
    let mut limiter = RateLimiter::new(RateLimits { per_minute: 1, per_day: 2 });
    let start = Instant::now();
    assert!(limiter.try_acquire(user(1), start));
    assert!(!limiter.try_acquire(user(1), start + Duration::from_secs(10)));
    assert!(limiter.try_acquire(user(1), start + Duration::from_secs(60)));
}

#[test]
fn base_url_gets_chat_completions_appended() {
    let url = |base| chat_completions_url(base, false).unwrap().to_string();
    assert_eq!(url("https://openrouter.ai/api/v1"), "https://openrouter.ai/api/v1/chat/completions");
    assert_eq!(url("https://api.openai.com/v1/"), "https://api.openai.com/v1/chat/completions");
    assert_eq!(url("http://127.0.0.1:8790/v1"), "http://127.0.0.1:8790/v1/chat/completions");
    assert_eq!(url("http://localhost:8790"), "http://localhost:8790/chat/completions");
    assert_eq!(url("http://[::1]:8790/v1"), "http://[::1]:8790/v1/chat/completions");
}

#[test]
fn plain_http_needs_loopback_or_explicit_permission() {
    assert_eq!(
        chat_completions_url("http://ai.example.com/v1", false),
        Err(AiConfigError::InsecureHttp("http://ai.example.com/v1".into()))
    );
    assert!(chat_completions_url("http://ai.example.com/v1", true).is_ok());
    assert!(matches!(chat_completions_url("ftp://ai.example.com", true), Err(AiConfigError::InvalidBaseUrl(_))));
    assert!(matches!(chat_completions_url("not a url", true), Err(AiConfigError::InvalidBaseUrl(_))));
}

#[test]
fn debug_output_never_contains_the_api_key() {
    let settings = OpenAiSettings {
        base_url: "https://openrouter.ai/api/v1".into(),
        api_key: "sk-secret-value".into(),
        model: "some/model".into(),
        extra_headers: vec![("X-Title".into(), "Quest".into())],
        allow_insecure_http: false,
        openrouter: None,
    };
    assert!(!format!("{settings:?}").contains("sk-secret-value"));
    let provider = OpenAiCompatible::new(settings).unwrap();
    let debug = format!("{provider:?}");
    assert!(!debug.contains("sk-secret-value"), "{debug}");
    assert!(debug.contains("some/model"));
}

#[test]
fn json_text_is_parsed_from_plain_fenced_or_wrapped_answers() {
    let expected = Some(json!({ "tasks": [] }));
    assert_eq!(parse_json_text(r#"{"tasks": []}"#), expected);
    assert_eq!(parse_json_text("```json\n{\"tasks\": []}\n```"), expected);
    assert_eq!(parse_json_text("```\n{\"tasks\": []}\n```\n"), expected);
    assert_eq!(parse_json_text("Here you go:\n{\"tasks\": []}\nDone."), expected);
    assert_eq!(parse_json_text("no json here"), None);
    assert_eq!(parse_json_text("[1, 2]"), None, "only objects are answers");
    assert_eq!(parse_json_text("} {"), None);
}

#[test]
fn answer_comes_from_content_with_usage() {
    let body = json!({
        "choices": [{ "message": { "role": "assistant", "content": "{\"tasks\": []}" } }],
        "usage": { "prompt_tokens": 12, "completion_tokens": 3, "total_tokens": 15 },
    });
    let completion = parse_answer(body.to_string().as_bytes()).unwrap();
    assert_eq!(completion.value, json!({ "tasks": [] }));
    assert_eq!(completion.usage, Some(Usage { prompt_tokens: 12, completion_tokens: 3, cost: None }));
}

#[test]
fn openrouter_reports_the_cost_in_usage() {
    let body = json!({
        "choices": [{ "message": { "content": "{\"tasks\": []}" } }],
        "usage": { "prompt_tokens": 903, "completion_tokens": 24, "cost": 0.0003309, "cost_details": {} },
    });
    let usage = parse_answer(body.to_string().as_bytes()).unwrap().usage.unwrap();
    assert_eq!(usage.cost, Some(0.0003309));
}

#[test]
fn answer_comes_from_the_tool_call_arguments_as_string_or_object() {
    for arguments in [json!("{\"tasks\": []}"), json!({ "tasks": [] })] {
        let body = json!({
            "choices": [{ "message": { "content": null, "tool_calls": [
                { "id": "call_1", "type": "function", "function": { "name": "quest_capture", "arguments": arguments } }
            ] } }],
        });
        let completion = parse_answer(body.to_string().as_bytes()).unwrap();
        assert_eq!(completion.value, json!({ "tasks": [] }));
        assert_eq!(completion.usage, None);
    }
}

#[test]
fn null_tool_calls_and_content_parts_are_accepted() {
    let body = json!({ "choices": [{ "message": { "content": "{\"tasks\": []}", "tool_calls": null } }] });
    assert_eq!(parse_answer(body.to_string().as_bytes()).unwrap().value, json!({ "tasks": [] }));
    let parts = json!([{ "type": "text", "text": "{\"tasks\": " }, { "type": "text", "text": "[]}" }]);
    let body = json!({ "choices": [{ "message": { "content": parts } }] });
    assert_eq!(parse_answer(body.to_string().as_bytes()).unwrap().value, json!({ "tasks": [] }));
}

#[test]
fn base_url_with_credentials_is_refused() {
    assert_eq!(
        chat_completions_url("https://user:secret@ai.example.com/v1", false),
        Err(AiConfigError::CredentialsInBaseUrl)
    );
}

#[test]
fn unusable_answers_are_invalid_responses() {
    let bodies = [
        "not json".to_owned(),
        json!({ "choices": [] }).to_string(),
        json!({ "choices": [{ "message": { "content": "Sorry, no." } }] }).to_string(),
        json!({ "choices": [{ "message": { "content": null } }] }).to_string(),
        json!({ "choices": [{ "message": { "content": "{}", "refusal": "I cannot help" } }] }).to_string(),
        json!({ "choices": [{ "message": { "content": null, "tool_calls": [{ "function": { "arguments": "" } }] } }] })
            .to_string(),
    ];
    for body in bodies {
        assert_eq!(parse_answer(body.as_bytes()), Err(ProviderError::InvalidResponse), "{body}");
    }
}

#[test]
fn json_schema_rejections_are_recognised() {
    let rejects = |body: &[u8]| rejects_json_schema(400, body, false);
    assert!(rejects(br#"{"error":{"message":"Invalid parameter: 'response_format' of type 'json_schema' is not supported with this model."}}"#));
    assert!(rejects(b"model does not support Structured Outputs"));
    assert!(!rejects(br#"{"error":{"message":"Invalid model"}}"#));
    assert!(
        !rejects(br#"{"error":{"message":"Invalid schema for response_format 'quest_capture': In context=(), 'required' is required"}}"#),
        "a broken schema must not switch to tool calling"
    );
}

#[test]
fn openrouter_without_a_provider_for_the_parameters_counts_as_rejecting_json_schema() {
    let no_endpoints = br#"{"error":{"message":"No endpoints found that can handle the requested parameters. To learn more about provider routing, visit: https://openrouter.ai/docs/guides/routing/provider-selection","code":404}}"#;
    assert!(rejects_json_schema(404, no_endpoints, true));
    assert!(!rejects_json_schema(404, no_endpoints, false), "only OpenRouter words it like this");
    assert!(!rejects_json_schema(404, br#"{"error":{"message":"No endpoints found for mock/model."}}"#, true));
    assert!(!rejects_json_schema(500, no_endpoints, true));
}

#[test]
fn openrouter_is_recognised_by_its_host() {
    assert!(is_openrouter("https://openrouter.ai/api/v1"));
    assert!(is_openrouter(" https://eu.OpenRouter.ai/api/v1 "));
    assert!(!is_openrouter("https://api.openai.com/v1"));
    assert!(!is_openrouter("https://openrouter.ai.example.com/v1"));
    assert!(!is_openrouter("http://127.0.0.1:8790/v1"));
    assert!(!is_openrouter("not a url"));
}

#[test]
fn reasoning_effort_parses_from_configuration() {
    assert_eq!("none".parse(), Ok(ReasoningEffort::None));
    assert_eq!(" Low ".parse(), Ok(ReasoningEffort::Low));
    assert_eq!("default".parse(), Ok(ReasoningEffort::ModelDefault));
    assert!("off".parse::<ReasoningEffort>().is_err());
    assert_eq!(ReasoningEffort::default(), ReasoningEffort::None, "capture needs no reasoning");
}

#[test]
fn http_statuses_map_to_provider_errors() {
    assert_eq!(status_error(429), ProviderError::RateLimited);
    assert_eq!(status_error(504), ProviderError::Timeout);
    assert_eq!(status_error(401), ProviderError::Rejected { status: 401 });
    assert_eq!(status_error(500), ProviderError::Rejected { status: 500 });
}

#[test]
fn openrouter_options_follow_the_base_url_unless_forced() {
    let options = |base_url, force, fallbacks: &[&str]| {
        let fallbacks = fallbacks.iter().map(|m| m.to_string()).collect();
        OpenRouterOptions::for_base_url(base_url, force, fallbacks, ReasoningEffort::None)
    };
    let expected = OpenRouterOptions { fallback_models: vec!["b/c".into()], reasoning: ReasoningEffort::None };
    assert_eq!(options("https://openrouter.ai/api/v1", None, &[" b/c ", ""]), Ok(Some(expected)));
    assert_eq!(options("https://api.openai.com/v1", None, &[]), Ok(None), "OpenAI gets no OpenRouter fields");
    assert_eq!(options("https://openrouter.ai/api/v1", Some(false), &[]), Ok(None));
    assert!(options("https://proxy.example.com/v1", Some(true), &[]).unwrap().is_some());
    assert_eq!(options("https://api.openai.com/v1", None, &["b/c"]), Err(AiConfigError::FallbackNeedsOpenRouter));
}
