//! A stand-in AI provider for trying AI capture end to end without a real one (and without sending
//! anything anywhere). It speaks the OpenAI Chat Completions API and always answers with the
//! proposal for the example text "Morgen Angebot für Müller schreiben … Heute von neun bis elf am
//! Relaunch-Konzept gearbeitet.", whatever the text: a task for tomorrow, a new quest
//! "Steuererklärung" with a task on Friday, and two hours of work today.
//!
//! Dates follow the client's "now" from the prompt; a quest whose title contains "Relaunch" in the
//! context is used for the first task and the work log.
//!
//! ```sh
//! cargo run -p quest-server --example ai_mock            # listens on 127.0.0.1:8790
//! QUEST_AI_MOCK_BIND=127.0.0.1:9000 cargo run -p quest-server --example ai_mock
//! ```
//! Point a server at it with `QUEST_AI_BASE_URL=http://127.0.0.1:8790/v1` (see the README).

use std::net::SocketAddr;

use axum::Json;
use axum::Router;
use axum::routing::post;
use chrono::{Datelike, Days, NaiveDate};
use serde_json::{Value, json};

#[tokio::main]
async fn main() {
    let bind: SocketAddr = std::env::var("QUEST_AI_MOCK_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8790".into())
        .parse()
        .expect("QUEST_AI_MOCK_BIND must be an address such as 127.0.0.1:8790");
    let router = Router::new().route("/v1/chat/completions", post(complete));
    let listener = tokio::net::TcpListener::bind(bind).await.expect("cannot listen; pick another QUEST_AI_MOCK_BIND");
    println!("ai_mock listening on http://{bind}/v1");
    axum::serve(listener, router).await.expect("mock server failed");
}

async fn complete(Json(request): Json<Value>) -> Json<Value> {
    let context = request["messages"]
        .as_array()
        .and_then(|messages| messages.iter().find(|message| message["role"] == "user"))
        .and_then(|message| message["content"].as_str())
        .and_then(context_of)
        .unwrap_or(Value::Null);
    let answer = answer(&context);
    let uses_tools = request.get("tools").is_some();
    println!("answered a capture request ({})", if uses_tools { "tool call" } else { "json_schema" });
    let message = if uses_tools {
        json!({ "role": "assistant", "content": null, "tool_calls": [{ "id": "call_mock", "type": "function",
            "function": { "name": request["tools"][0]["function"]["name"], "arguments": answer.to_string() } }] })
    } else {
        json!({ "role": "assistant", "content": answer.to_string() })
    };
    Json(json!({
        "id": "chatcmpl-mock",
        "object": "chat.completion",
        "choices": [{ "index": 0, "finish_reason": "stop", "message": message }],
        "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 },
    }))
}

/// The context JSON `quest_capture::prompt` puts between "Context (JSON):" and the text.
fn context_of(user_message: &str) -> Option<Value> {
    let start = user_message.find('{')?;
    let end = user_message.find("\n\nText to structure")?;
    serde_json::from_str(user_message.get(start..end)?).ok()
}

fn answer(context: &Value) -> Value {
    let today = context["now"]["date"]
        .as_str()
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Local::now().date_naive());
    let tomorrow = today + Days::new(1);
    let days_to_friday = (4 + 7 - u64::from(today.weekday().num_days_from_monday())) % 7;
    let friday = today + Days::new(if days_to_friday == 0 { 7 } else { days_to_friday });
    let relaunch = context["quests"].as_array().and_then(|quests| {
        quests.iter().find(|quest| quest["title"].as_str().is_some_and(|t| t.to_lowercase().contains("relaunch")))
    });
    let relaunch_id = relaunch.map(|quest| quest["id"].clone()).unwrap_or(Value::Null);
    let confidence = if relaunch.is_some() { "high" } else { "low" };
    let date = |day: NaiveDate| day.format("%Y-%m-%d").to_string();
    json!({
        "new_quests": [{ "key": "q1", "title": "Steuererklärung" }],
        "tasks": [
            { "title": "Angebot für Müller schreiben", "description": "Dauert etwa zwei Stunden.",
              "quest_id": relaunch_id, "new_quest_key": null, "quest_confidence": confidence,
              "planned_on": date(tomorrow), "due_on": null, "repeat": null },
            { "title": "Belege sammeln", "description": "", "quest_id": null, "new_quest_key": "q1",
              "quest_confidence": "high", "planned_on": date(friday), "due_on": null, "repeat": null },
        ],
        "work_logs": [
            { "task_id": null, "new_task_title": "Relaunch-Konzept", "quest_id": relaunch_id,
              "new_quest_key": null, "confidence": confidence, "date": date(today), "start_time": "09:00",
              "end_time": "11:00" },
        ],
    })
}
