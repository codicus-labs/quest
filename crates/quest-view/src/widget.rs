use chrono::{DateTime, Utc};
use quest_domain::{Command, EntityId, EntryKind, RunningFocus, Snapshot};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Mode {
    #[default]
    Card,
    Pill,
    Hidden,
}

pub fn visible(mode: Mode, main_focused: bool, has_timer: bool) -> bool {
    mode != Mode::Hidden && !main_focused && has_timer
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pause {
    pub workspace: EntityId,
    pub task: EntityId,
    entry: EntityId,
    ended_at: DateTime<Utc>,
    pub elapsed_ms: i64,
}

impl Pause {
    pub fn stop(running: &RunningFocus, at: DateTime<Utc>) -> Option<(Self, Command)> {
        let entry = &running.entry;
        if entry.kind != EntryKind::Task {
            return None;
        }
        let at = at.max(entry.started_at);
        let paused = Self {
            workspace: running.workspace,
            task: entry.task_id?,
            entry: entry.id,
            ended_at: at,
            elapsed_ms: (at - entry.started_at).num_milliseconds().max(0),
        };
        Some((paused, Command::StopFocus { at }))
    }

    pub fn valid(self, snapshot: &Snapshot) -> bool {
        snapshot.workspace == self.workspace
            && snapshot.running_focus().is_none()
            && snapshot.task(self.task).is_some_and(|task| !task.is_done())
            && snapshot.time_entries.iter().any(|entry| entry.id == self.entry && entry.ended_at == Some(self.ended_at))
            && !snapshot.time_entries.iter().any(|entry| entry.started_at > self.ended_at)
    }

    pub fn resume(self, snapshot: &Snapshot, entry_id: EntityId, at: DateTime<Utc>) -> Option<Command> {
        self.valid(snapshot).then_some(Command::StartFocus { entry_id, task_id: self.task, at })
    }
}

#[derive(Clone, Copy)]
pub struct Screen {
    pub origin: (i32, i32),
    pub size: (i32, i32),
    pub scale: f64,
}

pub fn clamp_position(position: (i32, i32), points: (f32, f32), screens: &[Screen]) -> (i32, i32) {
    let clamp = |value: i32, origin: i32, span: i32, extent: f32, scale: f64| {
        value.clamp(origin, origin + (span - (f64::from(extent) * scale).ceil() as i32).max(0))
    };
    screens
        .iter()
        .map(|s| {
            let x = clamp(position.0, s.origin.0, s.size.0, points.0, s.scale);
            let y = clamp(position.1, s.origin.1, s.size.1, points.1, s.scale);
            (x, y)
        })
        .min_by_key(|&(x, y)| (f64::from(x) - f64::from(position.0)).hypot(f64::from(y) - f64::from(position.1)) as u64)
        .unwrap_or(position)
}

#[cfg(test)]
mod tests;
