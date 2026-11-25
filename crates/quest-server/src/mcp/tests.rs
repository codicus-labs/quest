use uuid::Uuid;

use super::*;
use quest_protocol::UserId;

fn principal(access: Access, ai_enabled: bool) -> McpPrincipal {
    McpPrincipal { user: UserId(Uuid::nil()), grant: Uuid::nil(), access, ai_enabled }
}

#[test]
fn read_only_tokens_see_read_tools_only() {
    let reader = principal(Access::Read, true);
    assert!(tool_allowed(&reader, "list_tasks"));
    assert!(!tool_allowed(&reader, "create_task"));
    assert!(!tool_allowed(&reader, CAPTURE_TOOL));
    let writer = principal(Access::Write, false);
    assert!(tool_allowed(&writer, "create_task"));
    assert!(!tool_allowed(&writer, CAPTURE_TOOL));
    assert!(tool_allowed(&principal(Access::Write, true), CAPTURE_TOOL));
}

#[test]
fn refusals_name_their_reason() {
    let code = |access, ai, tool| tool_refusal(&principal(access, ai), tool).map(|error| error.code);
    assert_eq!(code(Access::Read, true, "create_task"), Some(ToolCode::InsufficientScope));
    assert_eq!(code(Access::Read, false, CAPTURE_TOOL), Some(ToolCode::InsufficientScope));
    assert_eq!(code(Access::Write, false, CAPTURE_TOOL), Some(ToolCode::AiDisabled));
    assert_eq!(code(Access::Write, true, CAPTURE_TOOL), None);
}

#[test]
fn every_read_tool_exists() {
    let names: Vec<String> = QuestMcp::tool_router().list_all().into_iter().map(|tool| tool.name.to_string()).collect();
    for tool in READ_TOOLS {
        assert!(names.iter().any(|name| name == tool), "{tool} is not a tool");
    }
}

#[test]
fn write_calls_are_found_in_single_messages_and_batches() {
    let call = |tool: &str| format!(r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"{tool}"}}}}"#);
    assert!(calls_write_tool(call("create_task").as_bytes()));
    assert!(!calls_write_tool(call("list_tasks").as_bytes()));
    assert!(calls_write_tool(format!("[{},{}]", call("list_tasks"), call("stop_timer")).as_bytes()));
    assert!(!calls_write_tool(br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#));
    assert!(!calls_write_tool(b"not json"));
}
