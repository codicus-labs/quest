//! AI capture: the panel's steps (text, structuring, review), the request off the
//! UI thread (`rune::Latest`), and applying the confirmed proposal as one unit that ⌘Z takes back.

use chrono::{DateTime, Duration, Utc};
use quest_capture::Created;
use quest_domain::EntityId;
use quest_protocol::{CaptureProposal, CaptureRequest, TempKey};
use quest_store::CaptureError;
use rune::{Latest, TextEdit};

use super::capture_draft::Review;
use super::{Overlay, State};
use crate::undo::TOAST_SECONDS;

/// Where proposals come from: the user's Quest server, or (for tests and the E2E run, without
/// network) a fixed proposal (`--capture-fixture`).
#[derive(Clone)]
pub enum CaptureSource {
    Server,
    Fixture(CaptureProposal),
    /// Tests: the fixed proposal arrives only after the test sends on the gate, so a test can act
    /// while the request is still running.
    #[cfg(test)]
    Gated(CaptureProposal, std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Receiver<()>>>),
}

pub enum CaptureStep {
    /// Typing (or dictating) the text; `error` says why the last attempt failed.
    Compose {
        error: Option<String>,
    },
    /// The request runs; Esc goes back to Compose and drops its answer.
    Structuring,
    Review(Review),
}

pub struct CaptureState {
    pub step: CaptureStep,
    pub text: TextEdit,
    pub source: CaptureSource,
    /// Takes edits for an item field whose proposal is already gone.
    scratch: TextEdit,
    answers: Latest<Result<CaptureProposal, CaptureError>>,
}

impl Default for CaptureState {
    fn default() -> CaptureState {
        CaptureState {
            step: CaptureStep::Compose { error: None },
            text: TextEdit::new(""),
            source: CaptureSource::Server,
            scratch: TextEdit::new(""),
            answers: Latest::default(),
        }
    }
}

impl CaptureState {
    /// Wakes the window when an answer lands.
    pub fn set_waker(&self, waker: rune::Waker) {
        self.answers.set_waker(waker);
    }

    pub fn review(&self) -> Option<&Review> {
        match &self.step {
            CaptureStep::Review(draft) => Some(draft),
            _ => None,
        }
    }

    /// The title field of a proposed item.
    pub fn item_title(&mut self, key: &TempKey) -> &mut TextEdit {
        let CaptureState { step, scratch, .. } = self;
        match step {
            CaptureStep::Review(review) => review.titles.get_mut(key).unwrap_or(scratch),
            _ => scratch,
        }
    }

    pub fn review_mut(&mut self) -> Option<&mut Review> {
        match &mut self.step {
            CaptureStep::Review(draft) => Some(draft),
            _ => None,
        }
    }
}

/// What the last capture created, for ⌘Z and the toast.
pub struct CaptureUndo {
    pub created: Vec<Created>,
    pub message: String,
    /// While the toast shows.
    pub until: Option<DateTime<Utc>>,
}

impl State {
    /// Whether AI entries show: the account turned AI features on (after the privacy notice)
    /// and its server has an AI provider.
    pub fn ai_capture_enabled(&self) -> bool {
        self.account.as_ref().is_some_and(|a| a.info.ai_enabled && a.info.ai_available)
    }

    /// Opens the panel at its text, or at the proposal it was closed on. Text from an earlier
    /// attempt stays.
    pub fn open_capture(&mut self) {
        self.capture.answers.cancel();
        if !matches!(self.capture.step, CaptureStep::Review(_)) {
            self.capture.step = CaptureStep::Compose { error: None };
        }
        self.menu = None;
        self.overlay = Overlay::Capture;
    }

    /// Esc: stops structuring (its answer is dropped), else closes the panel.
    pub fn capture_back(&mut self) {
        if matches!(self.capture.step, CaptureStep::Structuring) {
            self.capture.answers.cancel();
            self.capture.step = CaptureStep::Compose { error: None };
            return;
        }
        self.close_capture();
    }

    /// Closes the panel; a proposal under review stays for the next time (it took a while to
    /// get), a running request is dropped.
    pub fn close_capture(&mut self) {
        self.quest_picker = None;
        self.cancel_capture();
        if !matches!(self.capture.step, CaptureStep::Review(_)) {
            self.capture.step = CaptureStep::Compose { error: None };
        }
        if self.overlay == Overlay::Capture {
            self.overlay = Overlay::None;
        }
    }

    /// "Cancel": closes the panel and drops the proposal.
    pub fn discard_capture(&mut self) {
        self.capture.step = CaptureStep::Compose { error: None };
        self.close_capture();
    }

    /// Drops a running request (also when the window closes: state outlives it).
    pub fn cancel_capture(&mut self) {
        self.capture.answers.cancel();
    }

    /// "Structure": sends the text with a compact context, off the UI thread.
    pub fn structure_capture(&mut self) {
        if !matches!(self.capture.step, CaptureStep::Compose { .. }) {
            return;
        }
        if self.capture.text.text.trim().is_empty() {
            self.capture.step = CaptureStep::Compose { error: Some("Type or dictate what to capture first.".into()) };
            return;
        }
        self.now = self.clock.now();
        let request =
            quest_capture::request(&self.capture.text.text, self.now, self.repeat_zone, &self.language, &self.snap);
        let work: Box<dyn FnOnce(CaptureRequest) -> Result<CaptureProposal, CaptureError> + Send> =
            match self.capture.source.clone() {
                CaptureSource::Server => {
                    let ai = self.store.ai();
                    Box::new(move |request| ai.capture(&request))
                }
                CaptureSource::Fixture(proposal) => Box::new(move |_| Ok(proposal)),
                #[cfg(test)]
                CaptureSource::Gated(proposal, gate) => Box::new(move |_| {
                    if let Ok(gate) = gate.lock() {
                        let _ = gate.recv();
                    }
                    Ok(proposal)
                }),
            };
        self.capture.answers.request(request, work);
        self.capture.step = CaptureStep::Structuring;
    }

    /// Takes in an answer that landed; true when the panel changed.
    pub(super) fn take_capture_answer(&mut self) -> bool {
        let Some(answer) = self.capture.answers.take() else { return false };
        if self.overlay != Overlay::Capture || !matches!(self.capture.step, CaptureStep::Structuring) {
            return false;
        }
        self.capture.step = match answer {
            Ok(proposal) => CaptureStep::Review(Review::new(proposal)),
            Err(error) => CaptureStep::Compose { error: Some(capitalized(&error.to_string())) },
        };
        true
    }

    /// Back from the proposal to the text, to change it and structure again.
    pub fn edit_capture_text(&mut self) {
        self.quest_picker = None;
        self.capture.step = CaptureStep::Compose { error: None };
    }

    /// "Create (n)": the selected items as one unit; afterwards a toast offers to undo it.
    pub fn create_from_capture(&mut self) {
        let Some(review) = self.capture.review() else { return };
        if !review.draft.can_apply(&self.snap) {
            return;
        }
        let confirmed = review.draft.confirmed();
        self.now = self.clock.now();
        let result = quest_capture::plan(&confirmed, &self.snap, self.now, self.repeat_zone, EntityId::new)
            .map_err(|error| error.to_string())
            .and_then(|plan| self.store.dispatch_all(&plan.commands).map(|()| plan).map_err(|error| error.to_string()));
        let plan = match result {
            Ok(plan) => plan,
            Err(error) => {
                if let Some(review) = self.capture.review_mut() {
                    review.error = Some(format!("Nothing was created: {error}."));
                }
                return;
            }
        };
        self.adopt_snapshot();
        self.sync_focus_draft();
        self.notice = None;
        self.capture.text = TextEdit::new("");
        self.discard_capture();
        self.undo = crate::undo::UndoState::default();
        self.capture_undo = Some(CaptureUndo {
            created: plan.created,
            message: Review::created_message(&confirmed),
            until: Some(self.now + Duration::seconds(TOAST_SECONDS)),
        });
    }

    /// ⌘Z or the toast's Undo: deletes what the last capture created (what still exists).
    pub fn undo_capture(&mut self) {
        let Some(undo) = self.capture_undo.take() else { return };
        self.now = self.clock.now();
        let commands = quest_capture::undo(&undo.created, &self.snap, self.now);
        if let Err(error) = self.store.dispatch_all(&commands) {
            self.notice = Some(format!("Undo didn’t work: {error}."));
        }
        self.adopt_snapshot();
        self.sync_focus_draft();
    }
}

/// Store errors read as the end of a sentence ("could not reach …"); the panel shows them alone.
pub(super) fn capitalized(message: &str) -> String {
    let mut chars = message.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}
