//! The account dialog's "Connected AI clients" without a server: nothing loads signed out.

use super::tests::demo_app;
use crate::state::Overlay;
use crate::state::ai_connections::ConnectionsView;

#[test]
fn signed_out_the_dialog_loads_nothing() {
    let (mut rt, _dir) = demo_app();
    rt.state.overlay = Overlay::Account;
    rt.frame(0.0);
    assert!(matches!(rt.state.ai_connections.view, ConnectionsView::NotLoaded));
    assert!(rt.find("account-ai-clients").is_none());
}

#[test]
fn closing_the_dialog_forgets_the_list() {
    let (mut rt, _dir) = demo_app();
    rt.state.ai_connections.view = ConnectionsView::Failed("Could not reach your Quest server".into());
    rt.frame(0.0);
    assert!(matches!(rt.state.ai_connections.view, ConnectionsView::NotLoaded));
}

#[test]
fn disconnect_asks_first_and_keep_takes_it_back() {
    let (mut rt, _dir) = demo_app();
    let connection: quest_store::AiConnection = serde_json::from_value(serde_json::json!({
        "id": "0196d1a0-0000-7000-8000-000000000001",
        "client_name": "Claude Code",
        "redirect_host": "claude.ai",
        "created_at": "2026-10-01T12:00:00Z",
        "last_used_at": null,
        "scope": "quest:read",
    }))
    .expect("a connection");
    rt.state.ask_revoke_ai_connection(&connection);
    assert_eq!(rt.state.ai_connections.confirming.as_ref().map(|c| c.id), Some(connection.id));
    rt.state.keep_ai_connection();
    assert!(rt.state.ai_connections.confirming.is_none());
    rt.state.revoke_ai_connection();
    assert!(matches!(rt.state.ai_connections.view, ConnectionsView::NotLoaded), "nothing confirmed, nothing sent");
    rt.state.ask_revoke_ai_connection(&connection);
    rt.state.revoke_ai_connection();
    assert!(rt.state.ai_connections.confirming.is_none());
    assert!(matches!(rt.state.ai_connections.view, ConnectionsView::Loading));
}
