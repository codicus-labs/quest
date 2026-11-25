//! Splits the outbox into pushes the server can accept. Pure.

use quest_domain::FieldChange;
use quest_protocol::{MAX_FRAME_BYTES, MAX_PUSH_CHANGES, MAX_VALUE_BYTES};

use crate::db::OutboxEntry;

/// Room for the message envelope (type tag, batch id, brackets).
const ENVELOPE_BYTES: usize = 1024;

#[derive(Debug, Default, PartialEq)]
pub(crate) struct BatchPlan {
    /// Rows to push now, oldest first.
    pub push: Vec<OutboxEntry>,
    /// Rows the server could never accept: their value alone exceeds the limit.
    pub reject: Vec<i64>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    pub changes: usize,
    pub frame_bytes: usize,
    pub value_bytes: usize,
}

impl Limits {
    pub const PROTOCOL: Limits =
        Limits { changes: MAX_PUSH_CHANGES, frame_bytes: MAX_FRAME_BYTES, value_bytes: MAX_VALUE_BYTES };
}

/// Takes rows in order while the batch stays within the limits. Oversized values are set aside
/// instead of blocking everything behind them.
pub(crate) fn plan_batch(rows: Vec<OutboxEntry>, limits: Limits) -> BatchPlan {
    let mut plan = BatchPlan::default();
    let mut frame_bytes = ENVELOPE_BYTES;
    for (seq, change) in rows {
        if value_bytes(&change) > limits.value_bytes {
            plan.reject.push(seq);
            continue;
        }
        let bytes = change_bytes(&change) + 1;
        let full = plan.push.len() >= limits.changes || frame_bytes + bytes > limits.frame_bytes;
        if full && !plan.push.is_empty() {
            break;
        }
        frame_bytes += bytes;
        plan.push.push((seq, change));
    }
    plan
}

fn value_bytes(change: &FieldChange) -> usize {
    change.value.to_string().len()
}

fn change_bytes(change: &FieldChange) -> usize {
    serde_json::to_string(change).map_or(usize::MAX / 2, |json| json.len())
}

#[cfg(test)]
mod tests {
    use quest_domain::{EntityId, EntityKind, Hlc};
    use serde_json::{Value, json};

    use super::*;

    fn row(seq: i64, value: Value) -> OutboxEntry {
        (
            seq,
            FieldChange {
                kind: EntityKind::Task,
                id: EntityId::SETTINGS,
                field: "title".into(),
                value,
                hlc: Hlc::ZERO,
            },
        )
    }

    fn seqs(plan: &BatchPlan) -> Vec<i64> {
        plan.push.iter().map(|(seq, _)| *seq).collect()
    }

    #[test]
    fn stops_at_the_change_limit() {
        let rows = (1..=5).map(|seq| row(seq, json!("x"))).collect();
        let plan = plan_batch(rows, Limits { changes: 3, ..Limits::PROTOCOL });
        assert_eq!(seqs(&plan), [1, 2, 3]);
    }

    #[test]
    fn stops_before_the_frame_would_get_too_big() {
        let big = json!("y".repeat(400));
        let rows = (1..=5).map(|seq| row(seq, big.clone())).collect();
        let plan = plan_batch(rows, Limits { frame_bytes: ENVELOPE_BYTES + 1200, ..Limits::PROTOCOL });
        assert_eq!(seqs(&plan), [1, 2]);
    }

    #[test]
    fn oversized_values_are_rejected_without_blocking_the_rest() {
        let rows = vec![row(1, json!("ok")), row(2, json!("z".repeat(100))), row(3, json!("fine"))];
        let plan = plan_batch(rows, Limits { value_bytes: 50, ..Limits::PROTOCOL });
        assert_eq!(seqs(&plan), [1, 3]);
        assert_eq!(plan.reject, [2]);
    }

    #[test]
    fn a_single_change_always_fits() {
        let plan = plan_batch(vec![row(1, json!("w".repeat(500)))], Limits { frame_bytes: 10, ..Limits::PROTOCOL });
        assert_eq!(seqs(&plan), [1]);
    }
}
