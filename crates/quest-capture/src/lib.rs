//! AI capture, pure: no IO, no clock (time is passed in). Shared by `quest-server` (prompt, schema,
//! validation of the model's answer) and the clients (context, applying a confirmed proposal).
//!
//! Flow: a client builds a [`quest_protocol::CaptureRequest`] with [`request`]; the server checks it
//! with [`check_request`], asks the model with [`prompt`] and [`output_schema`] and turns the answer
//! into a [`quest_protocol::CaptureProposal`] with [`validate`], treating it as untrusted input. The
//! client shows the proposal, the user edits and confirms it, and [`plan`] turns the confirmed part
//! into domain commands (applied all or nothing; [`undo`] takes them back).

mod apply;
mod context;
mod prompt;
pub mod review;
#[cfg(test)]
mod test_support;
mod text;
mod validate;

pub use apply::{ApplyError, CapturePlan, Created, instant, new_quest_colors, plan, undo};
pub use context::{context, request};
pub use prompt::{Prompt, RequestError, SCHEMA_NAME, check_request, output_schema, prompt};
pub use review::Draft;
pub use validate::{InvalidOutput, validate};
