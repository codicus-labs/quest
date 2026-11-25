//! `QuestCore` for AI capture: asking the server, showing the draft, applying it as one unit.

use std::sync::{MutexGuard, PoisonError};

use quest_domain::EntityId;

use crate::capture::{self, CaptureUndo};
use crate::capture_types::{CaptureApplied, CaptureDraft, CaptureView};
use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::time::Now;
use crate::types::Moment;

#[uniffi::export]
impl QuestCore {
    /// Sends `text` with a compact context (active quests, tasks around today) to the Quest server,
    /// which structures it with its AI provider. Blocks on the network for up to about 75 s: never
    /// on the UI thread. Other calls on the core go on meanwhile. `language`: BCP 47, e.g. "de-DE".
    pub fn capture(&self, text: String, language: String, now: Moment) -> Result<CaptureDraft, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let (ai, snapshot) = {
                let store = self.store();
                (store.ai(), store.snapshot())
            };
            let request = quest_capture::request(&text, now.at, now.tz, &language, &snapshot);
            let proposal = ai.capture(&request).map_err(capture::capture_error)?;
            Ok(capture::draft(&proposal, &self.store().snapshot(), now))
        })
    }

    /// How the (edited) draft reads: labels, quest chips, errors, "Create (n)". Memory-only.
    pub fn capture_view(&self, draft: CaptureDraft, now: Moment) -> Result<CaptureView, MobileError> {
        guard(|| capture::view(&draft, &self.store().snapshot(), Now::parse(&now)?))
    }

    /// Creates everything selected in `draft` as one unit: all of it, or nothing if one part is
    /// refused. Remembers what it created for `undo_capture`, replacing an earlier capture's.
    /// Memory-only.
    pub fn apply_capture(&self, draft: CaptureDraft, now: Moment) -> Result<CaptureApplied, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let (plan, message) = self.edit(|store| {
                let (plan, message) = capture::apply(&draft, &store.snapshot(), now, EntityId::new)?;
                store.dispatch_all(&plan.commands).map_err(|e| MobileError::Rejected { reason: e.to_string() })?;
                Ok((plan, message))
            })?;
            *self.capture_undo_state() = Some(CaptureUndo::new(plan.created, now));
            Ok(capture::applied(message))
        })
    }

    /// The Undo of the snackbar after `apply_capture`: deletes softly what that capture created
    /// and still exists (as one unit), like the desktop's ⌘Z. Only once, and only for a while
    /// after the capture (`capture::UNDO_LIMIT`); otherwise, and after signing out, it does
    /// nothing. Memory-only.
    pub fn undo_capture(&self, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let Some(undo) = self.capture_undo_state().take() else { return Ok(()) };
            let Some(created) = undo.created_if_open(now) else { return Ok(()) };
            self.edit(|store| {
                let commands = quest_capture::undo(&created, &store.snapshot(), now.at);
                store.dispatch_all(&commands).map_err(|e| MobileError::Rejected { reason: e.to_string() })
            })
        })
    }

    /// Retries a lost sync connection now instead of after its backoff, e.g. when the network is
    /// back. Memory-only.
    pub fn reconnect_now(&self) {
        self.store().reconnect_now();
    }
}

impl QuestCore {
    fn capture_undo_state(&self) -> MutexGuard<'_, Option<CaptureUndo>> {
        self.capture_undo.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
