//! Test helpers shared by the domain's test modules.

use crate::change::{EntityId, FieldChange, FieldStore};
use crate::command::{Command, CommandError, plan};
use crate::hlc::{HlcClock, NodeId};
use crate::snapshot::Snapshot;

/// Applies commands the way a store does: plan, stamp, merge, rebuild the snapshot.
pub(crate) struct Replica {
    pub(crate) store: FieldStore,
    pub(crate) clock: HlcClock,
    pub(crate) snapshot: Snapshot,
    /// Every change this replica made, to merge into another one.
    pub(crate) changes: Vec<FieldChange>,
    ms: u64,
}

impl Replica {
    pub(crate) fn new() -> Self {
        Replica::on_node(1)
    }

    pub(crate) fn on_node(node: u64) -> Self {
        Replica {
            store: FieldStore::default(),
            clock: HlcClock::new(NodeId(node)),
            snapshot: Snapshot::default(),
            changes: Vec::new(),
            ms: 0,
        }
    }

    pub(crate) fn run(&mut self, command: Command) {
        self.try_run(command).expect("command should be valid");
    }

    pub(crate) fn try_run(&mut self, command: Command) -> Result<(), CommandError> {
        let writes = plan(&command, &self.snapshot)?;
        self.ms += 1;
        for w in writes {
            let change = w.stamp(self.clock.now(self.ms));
            self.store.apply(&change);
            self.changes.push(change);
        }
        self.switch(self.snapshot.workspace);
        Ok(())
    }

    /// Shows `workspace` (or the default one if it is not there).
    pub(crate) fn switch(&mut self, workspace: EntityId) {
        self.snapshot = Snapshot::from_store(&self.store, workspace);
    }

    /// Receives everything `other` changed, as sync would.
    pub(crate) fn merge(&mut self, other: &Replica) {
        for change in &other.changes {
            self.clock.observe(change.hlc, self.ms, u64::MAX);
            self.store.apply(change);
        }
        self.switch(self.snapshot.workspace);
    }
}
