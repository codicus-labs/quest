//! A local OpenAI-compatible Chat Completions endpoint that plays scripted replies and records the
//! requests it got.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::{Value, json};
use tokio::task::JoinHandle;

pub enum Reply {
    Status(u16, Value),
    /// Answers only after this long (the caller should have given up by then).
    Slow(Duration),
}

impl Reply {
    /// A successful completion whose message content is `content`.
    pub fn content(content: &str) -> Reply {
        Reply::Status(
            200,
            json!({
                "id": "chatcmpl-mock",
                "object": "chat.completion",
                "choices": [{ "index": 0, "finish_reason": "stop",
                    "message": { "role": "assistant", "content": content } }],
                "usage": { "prompt_tokens": 120, "completion_tokens": 40, "total_tokens": 160 },
            }),
        )
    }

    /// A successful completion that calls the tool `name` with `arguments` (as a JSON string).
    pub fn tool_call(name: &str, arguments: &Value) -> Reply {
        Reply::Status(
            200,
            json!({
                "choices": [{ "index": 0, "finish_reason": "tool_calls", "message": {
                    "role": "assistant", "content": null,
                    "tool_calls": [{ "id": "call_1", "type": "function",
                        "function": { "name": name, "arguments": arguments.to_string() } }],
                } }],
                "usage": { "prompt_tokens": 130, "completion_tokens": 45, "total_tokens": 175 },
            }),
        )
    }

    pub fn json_schema_unsupported() -> Reply {
        Reply::Status(
            400,
            json!({ "error": { "message": "Invalid parameter: 'response_format' of type 'json_schema' is not supported with this model.", "type": "invalid_request_error" } }),
        )
    }

    /// OpenRouter with `provider.require_parameters` when no provider of the model supports them all.
    pub fn no_endpoints_for_parameters() -> Reply {
        Reply::Status(
            404,
            json!({ "error": { "message": "No endpoints found that can handle the requested parameters. To learn more about provider routing, visit: https://openrouter.ai/docs/guides/routing/provider-selection", "code": 404 } }),
        )
    }
}

#[derive(Clone, Debug)]
pub struct Recorded {
    pub headers: HeaderMap,
    pub body: Value,
}

#[derive(Clone, Default)]
struct Script {
    replies: Arc<Mutex<VecDeque<Reply>>>,
    recorded: Arc<Mutex<Vec<Recorded>>>,
}

pub struct MockProvider {
    pub addr: SocketAddr,
    script: Script,
    task: JoinHandle<()>,
}

impl MockProvider {
    pub async fn start(replies: Vec<Reply>) -> MockProvider {
        let script = Script { replies: Arc::new(Mutex::new(replies.into())), ..Script::default() };
        let router = Router::new().route("/v1/chat/completions", post(complete)).with_state(script.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        MockProvider { addr, script, task }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    pub fn recorded(&self) -> Vec<Recorded> {
        self.script.recorded.lock().unwrap().clone()
    }
}

impl Drop for MockProvider {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn complete(State(script): State<Script>, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    script.recorded.lock().unwrap().push(Recorded { headers, body });
    let reply = script.replies.lock().unwrap().pop_front();
    match reply {
        Some(Reply::Status(status, body)) => (StatusCode::from_u16(status).unwrap(), Json(body)).into_response(),
        Some(Reply::Slow(delay)) => {
            tokio::time::sleep(delay).await;
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        None => (StatusCode::INTERNAL_SERVER_ERROR, "no scripted reply left").into_response(),
    }
}
