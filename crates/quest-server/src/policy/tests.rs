use quest_domain::{EntityId, EntityKind, Hlc, NodeId};
use serde_json::{Value, json};
use uuid::Uuid;

use super::*;

const NODE: NodeId = NodeId(1);
const NOW_MS: u64 = 1_000_000;

fn change(field: &str, value: Value, wall_ms: u64) -> FieldChange {
    FieldChange {
        kind: EntityKind::Task,
        id: EntityId::SETTINGS,
        field: field.into(),
        value,
        hlc: Hlc { wall_ms, counter: 0, node: NODE },
    }
}

fn sequenced(seq: u64) -> Sequenced {
    Sequenced { seq: Cursor(seq), change: change(&format!("f{seq}"), json!(seq), seq) }
}

fn rejected_message(changes: &[FieldChange]) -> String {
    let error = validate_push(changes, NODE, NOW_MS).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidRequest);
    error.message
}

#[test]
fn live_frames_skip_changes_already_sent_by_catch_up() {
    let batch = [sequenced(4), sequenced(5), sequenced(6)];
    let frames = live_frames(&batch, Cursor(5));
    assert_eq!(
        frames,
        vec![ServerMessage::Changes { changes: vec![batch[2].change.clone()], cursor: Cursor(6), caught_up: true }]
    );
}

#[test]
fn live_frames_are_empty_when_everything_was_sent() {
    assert!(live_frames(&[sequenced(3)], Cursor(3)).is_empty());
}

#[test]
fn frames_are_cut_by_size() {
    let big = json!("x".repeat(MAX_VALUE_BYTES - 10));
    let batch: Vec<Sequenced> =
        (1..=100).map(|seq| Sequenced { seq: Cursor(seq), change: change("title", big.clone(), seq) }).collect();
    let frames = live_frames(&batch, Cursor(0));
    assert!(frames.len() > 1);
    let mut seen = 0;
    for frame in &frames {
        assert!(serde_json::to_string(frame).unwrap().len() <= MAX_FRAME_BYTES);
        let ServerMessage::Changes { changes, cursor, .. } = frame else { panic!("expected changes") };
        seen += changes.len();
        assert_eq!(*cursor, Cursor(seen as u64));
    }
    assert_eq!(seen, 100);
    assert_eq!(frame_len(&batch, 3), 3, "count limit applies too");
}

#[test]
fn push_over_the_change_limit_is_rejected() {
    let changes: Vec<FieldChange> = (0..=MAX_PUSH_CHANGES as u64).map(|n| change("title", json!(n), n)).collect();
    rejected_message(&changes);
    assert!(validate_push(&changes[..MAX_PUSH_CHANGES], NODE, NOW_MS).is_ok());
}

#[test]
fn empty_field_name_is_rejected() {
    rejected_message(&[change("", json!(1), 1)]);
}

#[test]
fn oversized_value_is_rejected() {
    rejected_message(&[change("title", json!("x".repeat(MAX_VALUE_BYTES)), 1)]);
    assert!(validate_push(&[change("title", json!("x".repeat(MAX_VALUE_BYTES - 2)), 1)], NODE, NOW_MS).is_ok());
}

#[test]
fn change_from_another_node_is_rejected() {
    let mut foreign = change("title", json!(1), 1);
    foreign.hlc.node = NodeId(2);
    assert!(rejected_message(&[foreign]).contains("node"));
}

#[test]
fn change_too_far_in_the_future_is_rejected() {
    assert!(rejected_message(&[change("title", json!(1), NOW_MS + MAX_CLOCK_DRIFT_MS + 1)]).contains("clock"));
    assert!(validate_push(&[change("title", json!(1), NOW_MS + MAX_CLOCK_DRIFT_MS)], NODE, NOW_MS).is_ok());
    assert!(rejected_message(&[change("title", json!(1), u64::MAX)]).contains("clock"));
}

#[test]
fn resume_resets_on_unknown_epoch_or_cursor_beyond_history() {
    let epoch = Epoch(Uuid::now_v7());
    let server = SequenceState { epoch, last: Cursor(10) };
    let reset = Resume { cursor_reset: true, from: Cursor(0) };
    assert_eq!(resume(Some(epoch), Cursor(7), &server), Resume { cursor_reset: false, from: Cursor(7) });
    assert_eq!(resume(Some(epoch), Cursor(10), &server), Resume { cursor_reset: false, from: Cursor(10) });
    assert_eq!(resume(Some(epoch), Cursor(11), &server), reset);
    assert_eq!(resume(Some(Epoch(Uuid::nil())), Cursor(7), &server), reset);
    assert_eq!(resume(None, Cursor(0), &server), reset);
}

#[test]
fn only_the_current_protocol_is_accepted() {
    assert!(check_protocol(PROTOCOL_VERSION).is_ok());
    assert_eq!(check_protocol(PROTOCOL_VERSION + 1).unwrap_err().code, ErrorCode::UnsupportedProtocol);
}

#[test]
fn malformed_push_with_readable_batch_is_rejected_per_batch() {
    let batch = BatchId(Uuid::now_v7());
    let push = json!({"type": "push", "batch": batch, "changes": [{"kind": "spaceship"}]}).to_string();
    let Decoded::InvalidPush { batch: rejected, error } = decode_client_message(&push) else {
        panic!("expected InvalidPush")
    };
    assert_eq!((rejected, error.code), (batch, ErrorCode::InvalidRequest));
    assert!(matches!(decode_client_message("{\"type\":\"dance\"}"), Decoded::Invalid(_)));
    assert!(matches!(decode_client_message("not json"), Decoded::Invalid(_)));
}
