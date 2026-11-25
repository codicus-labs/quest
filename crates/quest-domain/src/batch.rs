//! Several commands as one unit: planned in order, stamped, all or nothing. Pure; the caller
//! passes the clock and the physical time and keeps the result only if it wants to commit.

use crate::change::{EntityId, FieldChange, FieldStore};
use crate::command::{Command, CommandError, plan};
use crate::hlc::HlcClock;
use crate::snapshot::Snapshot;

/// The outcome of [`plan_all`]: the stamped changes and the state after them.
#[derive(Clone, Debug)]
pub struct PlannedBatch {
    pub changes: Vec<FieldChange>,
    pub store: FieldStore,
    pub clock: HlcClock,
}

/// Plans each command against the state the ones before it leave in `workspace` (see
/// `Snapshot::from_store`) and stamps its writes with `clock` at `physical_ms`. If one is refused,
/// nothing is planned (the inputs are untouched).
pub fn plan_all(
    commands: &[Command],
    store: &FieldStore,
    workspace: EntityId,
    clock: &HlcClock,
    physical_ms: u64,
) -> Result<PlannedBatch, CommandError> {
    let mut store = store.clone();
    let mut clock = clock.clone();
    let mut changes = Vec::new();
    for command in commands {
        for write in plan(command, &Snapshot::from_store(&store, workspace))? {
            let change = write.stamp(clock.now(physical_ms));
            store.apply(&change);
            changes.push(change);
        }
    }
    Ok(PlannedBatch { changes, store, clock })
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod tests;
