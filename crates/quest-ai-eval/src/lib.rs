//! Eval set for AI capture. `cases.json` holds synthetic German dictations and the proposal each
//! should produce; [`score`] compares a validated proposal with that, [`stats`] sums up a run.
//! The binary (`mise run ai-eval --model <id>`) sends every case through quest-server's provider
//! to a real model. Only synthetic texts: nothing of a user's data goes into the eval.

pub mod cases;
pub mod score;
pub mod stats;
