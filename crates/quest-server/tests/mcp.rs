//! The MCP tools through a real MCP client (rmcp, Streamable HTTP) with an OAuth access token:
//! each tool once with success and once with a validation error, annotations, workspaces, and a
//! change that reaches a second device live over sync.

mod common;

use std::sync::Arc;

use chrono::{Days, Utc};
use common::TestServer;
use common::oauth::OAuthClient;
use futures_util::future::BoxFuture;
use quest_domain::{EntityKind, fields};
use quest_server::{AiConfig, Completion, LlmProvider, McpConfig, ProviderError, StructuredRequest};
use rmcp::model::{CallToolRequestParams, Tool};
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Value, json};

const USER: &str = "grace";
const ZONE: &str = "Europe/Berlin";

/// Answers every capture with one new task for tomorrow.
#[derive(Debug)]
struct FakeProvider;

impl LlmProvider for FakeProvider {
    fn complete_structured<'a>(&'a self, _: &'a StructuredRequest) -> BoxFuture<'a, Result<Completion, ProviderError>> {
        let tomorrow = Utc::now().with_timezone(&chrono_tz::Europe::Berlin).date_naive() + Days::new(1);
        let value = json!({
            "new_quests": [],
            "tasks": [{ "title": "Buy milk", "description": "", "quest_id": null, "new_quest_key": null,
                "quest_confidence": "high", "planned_on": tomorrow.to_string(), "due_on": null,
                "repeat": null }],
            "work_logs": [],
        });
        Box::pin(async move { Ok(Completion { value, usage: None }) })
    }
}

type Client = RunningService<RoleClient, ()>;

struct Setup {
    server: TestServer,
    client: Client,
}

impl Setup {
    /// With `ai`, AI capture is configured and the account has opted in.
    async fn new(ai: bool) -> Setup {
        Setup::with(ai, ai, None).await
    }

    async fn with(ai_configured: bool, ai_enabled: bool, access: Option<&str>) -> Setup {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let ai = ai_configured.then(|| AiConfig::new(Arc::new(FakeProvider)));
        let server = TestServer::start_with_mcp(McpConfig::default(), ai).await;
        let session = server.sign_in(USER).await;
        if ai_enabled {
            server.set_ai_enabled(&session.token, true).await;
        }
        let tokens = OAuthClient::register(&server).await.tokens_with(USER, access).await;
        let config = StreamableHttpClientTransportConfig::with_uri(server.url("/mcp")).auth_header(tokens.access);
        let client = ().serve(StreamableHttpClientTransport::from_config(config)).await.expect("MCP handshake");
        Setup { server, client }
    }

    /// The structured result, or the error's (code, message).
    async fn call(&self, tool: &str, mut arguments: Value) -> Result<Value, (String, String)> {
        if arguments.get("time_zone").is_none() {
            arguments["time_zone"] = json!(ZONE);
        }
        let params = CallToolRequestParams::new(tool.to_owned()).with_arguments(arguments.as_object().unwrap().clone());
        let result = self.client.call_tool(params).await.unwrap_or_else(|error| panic!("{tool}: {error}"));
        if result.is_error == Some(true) {
            let text = result.content[0].as_text().unwrap().text.clone();
            let error: Value =
                serde_json::from_str(&text).unwrap_or_else(|_| panic!("{tool}: error is not JSON: {text}"));
            return Err((error["code"].as_str().unwrap().to_owned(), error["message"].as_str().unwrap().to_owned()));
        }
        Ok(result.structured_content.unwrap_or_else(|| panic!("{tool}: no structured content")))
    }

    async fn ok(&self, tool: &str, arguments: Value) -> Value {
        self.call(tool, arguments).await.unwrap_or_else(|error| panic!("{tool}: {error:?}"))
    }

    /// Creates a workspace the way an app does (a sync push) and returns its id.
    async fn create_workspace(&self, name: &str) -> String {
        let session = self.server.sign_in(USER).await;
        let (mut device, _) = self.server.connect(&session, None).await;
        let id = quest_domain::EntityId::new();
        let mut change = common::change(id, fields::WORKSPACE_NAME, json!(name), 1);
        change.kind = EntityKind::Workspace;
        device.push(vec![change]).await;
        id.to_string()
    }

    async fn delete_workspace(&self, id: quest_domain::EntityId) {
        let session = self.server.sign_in(USER).await;
        let (mut device, _) = self.server.connect(&session, None).await;
        let mut change = common::change(id, fields::DELETED_AT, json!(Utc::now()), 2);
        change.kind = EntityKind::Workspace;
        device.push(vec![change]).await;
    }

    /// Asserts the tool refuses `arguments` with `code`.
    async fn refuses(&self, tool: &str, arguments: Value, code: &str) {
        match self.call(tool, arguments).await {
            Err((actual, message)) => {
                assert_eq!(actual, code, "{tool}: {message}");
                assert!(!message.is_empty());
            }
            Ok(value) => panic!("{tool} should fail with {code}, got {value}"),
        }
    }
}

fn today() -> String {
    Utc::now().with_timezone(&chrono_tz::Europe::Berlin).date_naive().to_string()
}

fn tool<'a>(tools: &'a [Tool], name: &str) -> &'a Tool {
    tools.iter().find(|tool| tool.name == name).unwrap_or_else(|| panic!("no tool {name}"))
}

#[tokio::test]
async fn tools_are_listed_with_schemas_and_honest_annotations() {
    let setup = Setup::new(false).await;
    let tools = setup.client.list_all_tools().await.unwrap();
    assert!(tools.iter().all(|tool| tool.name != "capture_text"), "capture_text needs AI");
    for name in [
        "list_tasks",
        "get_task",
        "get_checklist",
        "list_quests",
        "get_quest",
        "search",
        "get_week_review",
        "get_running_timer",
        "list_time_entries",
        "list_workspaces",
    ] {
        let annotations = tool(&tools, name).annotations.as_ref().unwrap();
        assert_eq!(annotations.read_only_hint, Some(true), "{name}");
    }
    for (name, destructive) in
        [("create_task", false), ("update_task", true), ("archive_task", true), ("complete_task", false)]
    {
        let annotations = tool(&tools, name).annotations.as_ref().unwrap();
        assert_eq!(
            (annotations.read_only_hint, annotations.destructive_hint),
            (Some(false), Some(destructive)),
            "{name}"
        );
    }
    for tool in &tools {
        assert!(tool.output_schema.is_some(), "{} has no output schema", tool.name);
        assert!(tool.input_schema.get("properties").is_some(), "{} has no input properties", tool.name);
    }
    let with_ai = Setup::new(true).await;
    let tools = with_ai.client.list_all_tools().await.unwrap();
    assert_eq!(tool(&tools, "capture_text").annotations.as_ref().unwrap().open_world_hint, Some(true));
}

#[tokio::test]
async fn capture_text_needs_the_accounts_ai_opt_in() {
    let s = Setup::with(true, false, None).await;
    let tools = s.client.list_all_tools().await.unwrap();
    assert!(tools.iter().all(|tool| tool.name != "capture_text"), "hidden without the opt-in");
    s.refuses("capture_text", json!({ "text": "Tomorrow buy milk" }), "ai_disabled").await;
}

#[tokio::test]
async fn read_only_access_lists_and_calls_only_reading_tools() {
    let s = Setup::with(true, true, Some("quest:read")).await;
    let tools = s.client.list_all_tools().await.unwrap();
    let mut names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    names.sort_unstable();
    let mut expected = vec![
        "get_checklist",
        "get_quest",
        "get_running_timer",
        "get_task",
        "get_week_review",
        "list_quests",
        "list_tasks",
        "list_time_entries",
        "list_workspaces",
        "search",
    ];
    expected.sort_unstable();
    assert_eq!(names, expected);
    assert!(s.ok("list_tasks", json!({ "unplanned": true })).await["tasks"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn task_tools() {
    let s = Setup::new(false).await;
    let quest = s.ok("create_quest", json!({ "title": "Website relaunch" })).await;
    let quest_id = quest["quest"]["id"].as_str().unwrap();

    let created = s
        .ok(
            "create_task",
            json!({ "title": "Write offer", "quest_id": quest_id, "planned_on": today(), "due_on": "2099-01-31" }),
        )
        .await;
    let task = &created["task"];
    let id = task["id"].as_str().unwrap().to_owned();
    assert_eq!((task["planned_on"].as_str(), task["due_on"].as_str()), (Some(today().as_str()), Some("2099-01-31")));
    assert_eq!(task["quest"]["title"], "Website relaunch");
    assert_eq!(created["time_zone"], ZONE);
    s.refuses("create_task", json!({ "title": " " }), "empty_title").await;
    s.refuses(
        "create_task",
        json!({ "title": "x", "planned_on": "2099-02-02", "due_on": "2099-02-01" }),
        "invalid_input",
    )
    .await;

    let listed = s.ok("list_tasks", json!({})).await;
    assert_eq!(listed["tasks"][0]["id"], id.as_str());
    s.refuses("list_tasks", json!({ "day": "tomorrow" }), "invalid_input").await;

    assert_eq!(s.ok("get_task", json!({ "task_id": id })).await["task"]["title"], "Write offer");
    s.refuses("get_task", json!({ "task_id": uuid::Uuid::now_v7().to_string() }), "task_not_found").await;

    let updated = s
        .ok("update_task", json!({ "task_id": id, "title": "Write offer for Müller", "repeat": { "frequency": "week", "weekdays": ["mon"] } }))
        .await;
    assert_eq!(updated["task"]["title"], "Write offer for Müller");
    assert_eq!(updated["task"]["repeat"], "Every week");
    s.refuses("update_task", json!({ "task_id": id }), "invalid_input").await;

    let moved = s.ok("move_task", json!({ "task_id": id, "planned_on": "2099-02-15" })).await;
    assert_eq!(moved["task"]["due_on"], "2099-02-15", "the deadline moves along");
    let unplanned = s.ok("move_task", json!({ "task_id": id, "planned_on": "none" })).await;
    assert!(unplanned["task"]["planned_on"].is_null());
    s.refuses("move_task", json!({ "task_id": id, "planned_on": "24.12." }), "invalid_input").await;
    let due = s.ok("update_task", json!({ "task_id": id, "due_on": "none" })).await;
    assert!(due["task"]["due_on"].is_null());

    let done = s.ok("complete_task", json!({ "task_id": id })).await;
    assert_eq!(done["task"]["done"], true);
    let completed_at = chrono::DateTime::parse_from_rfc3339(done["task"]["completed_at"].as_str().unwrap()).unwrap();
    assert_ne!(completed_at.offset().local_minus_utc(), 0, "instants carry the caller's offset");
    s.refuses("complete_task", json!({ "task_id": "nope" }), "invalid_input").await;

    assert_eq!(s.ok("reopen_task", json!({ "task_id": id })).await["task"]["done"], false);
    s.refuses("reopen_task", json!({ "task_id": uuid::Uuid::now_v7().to_string() }), "task_not_found").await;

    let archived = s.ok("archive_task", json!({ "task_id": id })).await;
    assert_eq!(archived["archived"]["kind"], "task");
    s.refuses("archive_task", json!({ "task_id": id }), "task_not_found").await;

    let found = s.ok("search", json!({ "query": "müller", "include_archived": true })).await;
    assert_eq!(found["archived"][0]["id"], id.as_str());
    s.refuses("search", json!({ "query": "  " }), "invalid_input").await;

    assert_eq!(s.ok("restore", json!({ "kind": "task", "id": id })).await["restored"], "task");
    s.refuses("restore", json!({ "kind": "task", "id": id }), "not_in_archive").await;
}

#[tokio::test]
async fn quest_tools() {
    let s = Setup::new(false).await;
    let quest = s.ok("create_quest", json!({ "title": "Marathon", "color": "teal" })).await;
    let quest_id = quest["quest"]["id"].as_str().unwrap().to_owned();
    assert_eq!(quest["quest"]["color"], "teal");
    s.refuses("create_quest", json!({ "title": "x", "color": "beige" }), "invalid_input").await;

    let updated = s.ok("update_quest", json!({ "quest_id": quest_id, "title": "Marathon 2027" })).await;
    assert_eq!(updated["quest"]["title"], "Marathon 2027");
    s.refuses("update_quest", json!({ "quest_id": quest_id, "title": "" }), "empty_title").await;

    s.ok("create_task", json!({ "title": "Long run", "quest_id": quest_id })).await;
    let detail = s.ok("get_quest", json!({ "quest_id": quest_id })).await;
    assert_eq!(detail["tasks"][0]["title"], "Long run");
    s.refuses("get_quest", json!({ "quest_id": uuid::Uuid::now_v7().to_string() }), "quest_not_found").await;

    let completed = s.ok("complete_quest", json!({ "quest_id": quest_id })).await;
    assert_eq!(completed["quest"]["status"], "completed");
    s.refuses("complete_quest", json!({}), "invalid_input").await;

    assert!(s.ok("list_quests", json!({})).await["quests"].as_array().unwrap().is_empty());
    assert_eq!(s.ok("list_quests", json!({ "status": "completed" })).await["quests"][0]["id"], quest_id.as_str());
    s.refuses("list_quests", json!({ "status": "paused" }), "invalid_input").await;
}

#[tokio::test]
async fn checklist_tools() {
    let s = Setup::new(false).await;
    let task = s.ok("create_task", json!({ "title": "Pack" })).await;
    let task_id = task["task"]["id"].as_str().unwrap().to_owned();

    assert!(s.ok("get_checklist", json!({ "task_id": task_id })).await["items"].as_array().unwrap().is_empty());
    s.refuses("get_checklist", json!({ "task_id": uuid::Uuid::now_v7().to_string() }), "task_not_found").await;

    s.ok("add_checklist_item", json!({ "task_id": task_id, "text": "Passport" })).await;
    let added = s.ok("add_checklist_item", json!({ "task_id": task_id, "text": "Charger" })).await;
    assert_eq!(added["task"]["title"], "Pack");
    let item = added["items"][0]["id"].as_str().unwrap().to_owned();
    s.refuses("add_checklist_item", json!({ "task_id": task_id, "text": " " }), "empty_title").await;

    let checked = s.ok("check_checklist_item", json!({ "item_id": item })).await;
    assert_eq!(checked["items"][0]["done"], true);
    let opened = s.ok("check_checklist_item", json!({ "item_id": item, "done": false })).await;
    assert_eq!(opened["items"][0]["done"], false);
    let unknown = uuid::Uuid::now_v7().to_string();
    s.refuses("check_checklist_item", json!({ "item_id": unknown }), "checklist_item_not_found").await;

    let renamed = s.ok("rename_checklist_item", json!({ "item_id": item, "text": "ID card" })).await;
    assert_eq!(renamed["items"][0]["text"], "ID card");
    s.refuses("rename_checklist_item", json!({ "item_id": "x", "text": "y" }), "invalid_input").await;

    let deleted = s.ok("delete_checklist_item", json!({ "item_id": item })).await;
    assert_eq!(deleted["items"][0]["text"], "Charger");
    s.refuses("delete_checklist_item", json!({ "item_id": item }), "checklist_item_not_found").await;
    assert_eq!(s.ok("restore", json!({ "kind": "checklist_item", "id": item })).await["restored"], "checklist_item");
    assert_eq!(s.ok("get_checklist", json!({ "task_id": task_id })).await["items"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn time_tools() {
    let s = Setup::new(false).await;
    let task = s.ok("create_task", json!({ "title": "Deep work" })).await;
    let task_id = task["task"]["id"].as_str().unwrap().to_owned();

    assert!(s.ok("get_running_timer", json!({})).await["running"].is_null());
    s.refuses("get_running_timer", json!({ "time_zone": "Moon/Base" }), "invalid_input").await;

    let started = s.ok("start_timer", json!({ "task_id": task_id })).await;
    assert_eq!((&started["entry"]["running"], &started["entry"]["kind"]), (&json!(true), &json!("task")));
    s.refuses("start_timer", json!({ "task_id": uuid::Uuid::now_v7().to_string() }), "task_not_found").await;
    assert_eq!(s.ok("get_running_timer", json!({})).await["running"]["task"]["id"], task_id.as_str());

    let organizing = s.ok("start_timer", json!({ "organizing": true, "task_id": task_id })).await;
    assert_eq!(organizing["entry"]["kind"], "organizing");
    assert_eq!(organizing["entry"]["origin_task"]["id"], task_id.as_str());
    s.refuses("start_timer", json!({}), "invalid_input").await;
    let running = s.ok("get_running_timer", json!({})).await;
    assert_eq!((&running["running"]["kind"], &running["running"]["task"]), (&json!("organizing"), &Value::Null));

    let stopped = s.ok("stop_timer", json!({})).await;
    assert_eq!((&stopped["stopped"]["running"], &stopped["stopped"]["kind"]), (&json!(false), &json!("organizing")));
    assert!(s.ok("stop_timer", json!({})).await["stopped"].is_null());
    s.refuses("stop_timer", json!({ "time_zone": "Nowhere" }), "invalid_input").await;

    let yesterday = (Utc::now().with_timezone(&chrono_tz::Europe::Berlin).date_naive() - Days::new(1)).to_string();
    let logged = s
        .ok("log_work", json!({ "new_task_title": "Concept", "day": yesterday, "start": "09:00", "end": "11:00" }))
        .await;
    assert_eq!(logged["entry"]["minutes"], 120);
    assert!(logged["entry"]["started_at"].as_str().unwrap().starts_with(&format!("{yesterday}T09:00:00+")));
    s.refuses(
        "log_work",
        json!({ "task_id": task_id, "day": yesterday, "start": "11:00", "end": "09:00" }),
        "invalid_time_range",
    )
    .await;

    let entries = s.ok("list_time_entries", json!({ "from": yesterday, "to": today() })).await;
    assert_eq!(entries["entries"].as_array().unwrap().len(), 3);
    s.refuses("list_time_entries", json!({ "from": today(), "to": yesterday }), "invalid_input").await;

    let review = s.ok("get_week_review", json!({ "week_of": yesterday })).await;
    assert!(review["other_minutes"].as_i64().unwrap() >= 120, "{review}");
    assert!(review["organizing_minutes"].is_i64(), "{review}");
    s.refuses("get_week_review", json!({ "week_of": "last week" }), "invalid_input").await;
}

#[tokio::test]
async fn capture_text_proposes_and_applies_all_or_nothing() {
    let s = Setup::new(true).await;
    let proposal = s.ok("capture_text", json!({ "text": "Tomorrow buy milk", "language": "en-US" })).await;
    assert_eq!(proposal["applied"], false);
    assert_eq!(proposal["proposal"]["tasks"][0]["title"], "Buy milk");
    assert!(s.ok("list_tasks", json!({ "unplanned": false })).await["tasks"].as_array().unwrap().is_empty());

    let applied = s.ok("capture_text", json!({ "text": "Tomorrow buy milk", "apply": true })).await;
    assert_eq!(applied["applied"], true);
    assert_eq!(applied["message"], "Created 1 task");
    let id = applied["created"][0]["id"].as_str().unwrap();
    assert_eq!(s.ok("get_task", json!({ "task_id": id })).await["task"]["title"], "Buy milk");

    s.refuses("capture_text", json!({ "text": "x".repeat(5000) }), "text_too_long").await;

    s.create_workspace("Home").await;
    let applied =
        s.ok("capture_text", json!({ "text": "Tomorrow buy milk", "apply": true, "workspace": "Home" })).await;
    let id = applied["created"][0]["id"].as_str().unwrap();
    assert_eq!(s.ok("get_task", json!({ "task_id": id, "workspace": "Home" })).await["task"]["title"], "Buy milk");
    s.refuses("get_task", json!({ "task_id": id }), "task_not_found").await;
    s.refuses("capture_text", json!({ "text": "Buy milk", "workspace": "Office" }), "workspace_not_found").await;
}

#[tokio::test]
async fn tools_work_in_the_default_workspace_unless_given_another() {
    let s = Setup::new(false).await;
    let work = s.create_workspace("Work").await;

    let listed = s.ok("list_workspaces", json!({})).await;
    let names: Vec<(&str, bool)> = listed["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|workspace| (workspace["name"].as_str().unwrap(), workspace["is_default"].as_bool().unwrap()))
        .collect();
    assert_eq!(names, [("Default", true), ("Work", false)]);
    assert_eq!(listed["workspaces"][1]["id"], work.as_str());

    let private = s.ok("create_task", json!({ "title": "Water plants" })).await["task"]["id"].clone();
    let release =
        s.ok("create_task", json!({ "title": "Ship release", "workspace": "work" })).await["task"]["id"].clone();

    let titles = |listed: Value| -> Vec<String> {
        listed["tasks"].as_array().unwrap().iter().map(|task| task["title"].as_str().unwrap().to_owned()).collect()
    };
    let unplanned = |workspace: Option<&str>| json!({ "unplanned": true, "workspace": workspace });
    assert_eq!(titles(s.ok("list_tasks", unplanned(None)).await), ["Water plants"]);
    assert_eq!(titles(s.ok("list_tasks", unplanned(Some(&work))).await), ["Ship release"]);
    s.refuses("get_task", json!({ "task_id": release }), "task_not_found").await;
    s.refuses("complete_task", json!({ "task_id": private, "workspace": "Work" }), "task_not_found").await;
    assert!(s.ok("search", json!({ "query": "release" })).await["tasks"].as_array().unwrap().is_empty());
    assert_eq!(s.ok("search", json!({ "query": "release", "workspace": "WORK" })).await["tasks"][0]["id"], release);

    let started = s.ok("start_timer", json!({ "task_id": release, "workspace": work })).await;
    assert_eq!(started["entry"]["running"], true);
    let running = s.ok("get_running_timer", json!({})).await;
    assert_eq!(running["running"]["task"]["id"], release, "one timer per account, seen from any workspace");
    assert_eq!(running["workspace"]["title"], "Work");
    s.ok("start_timer", json!({ "task_id": private })).await;
    let running = s.ok("get_running_timer", json!({ "workspace": "Work" })).await;
    assert_eq!(
        (&running["running"]["task"]["id"], running["workspace"]["title"].as_str()),
        (&private, Some("Default")),
        "starting in the default workspace stopped the one in Work"
    );
    let stopped = s.ok("stop_timer", json!({ "workspace": work })).await;
    assert_eq!(
        (&stopped["stopped"]["task"]["id"], stopped["workspace"]["title"].as_str()),
        (&private, Some("Default"))
    );

    let listed = s.ok("list_workspaces", json!({})).await;
    assert_eq!(
        (listed["workspaces"][0]["tasks"].as_u64(), listed["workspaces"][1]["tasks"].as_u64()),
        (Some(1), Some(1))
    );

    s.refuses("list_tasks", json!({ "workspace": "Nope" }), "workspace_not_found").await;
    s.refuses(
        "create_task",
        json!({ "title": "x", "workspace": uuid::Uuid::now_v7().to_string() }),
        "workspace_not_found",
    )
    .await;
}

#[tokio::test]
async fn without_a_workspace_tools_explain_that_the_default_one_is_deleted() {
    let s = Setup::new(false).await;
    s.ok("create_task", json!({ "title": "Old default task" })).await;
    s.create_workspace("Work").await;
    s.delete_workspace(quest_domain::EntityId::DEFAULT_WORKSPACE).await;

    s.refuses("list_tasks", json!({}), "workspace_not_found").await;
    let Err((_, message)) = s.call("create_task", json!({ "title": "x" })).await else { panic!("should fail") };
    assert!(message.contains("list_workspaces") && message.contains("workspace"), "{message}");
    let listed = s.ok("list_workspaces", json!({})).await;
    assert_eq!(listed["workspaces"].as_array().unwrap().len(), 1, "only Work is live");
    s.ok("create_task", json!({ "title": "Ship", "workspace": "Work" })).await;
}

#[tokio::test]
async fn wrong_argument_types_get_a_coded_error() {
    let s = Setup::new(false).await;
    s.refuses("create_task", json!({ "title": 42 }), "invalid_input").await;
}

#[tokio::test]
async fn changes_reach_other_devices_live() {
    let s = Setup::new(false).await;
    let session = s.server.sign_in(USER).await;
    let (mut device, _) = s.server.connect(&session, None).await;
    let created = s.ok("create_task", json!({ "title": "From the assistant" })).await;
    let id = created["task"]["id"].as_str().unwrap();
    let (changes, _) = device.next_changes().await;
    let title = changes
        .iter()
        .find(|change| change.kind == EntityKind::Task && change.id.to_string() == id && change.field == fields::TITLE)
        .expect("the title arrives live");
    assert_eq!(title.value, json!("From the assistant"));

    // And what a device pushed is what MCP reads.
    let pushed = common::change(quest_domain::EntityId::new(), fields::TITLE, json!("Pushed"), 1);
    let pushed_id = pushed.id;
    device
        .push(vec![
            pushed,
            common::change(pushed_id, fields::SORT_KEY, json!("zz"), 1),
            common::change(pushed_id, fields::TASK_PLANNED_ON, json!(null), 1),
        ])
        .await;
    let unplanned = s.ok("list_tasks", json!({ "unplanned": true })).await;
    assert!(unplanned["tasks"].as_array().unwrap().iter().any(|task| task["title"] == "Pushed"), "{unplanned}");
}
