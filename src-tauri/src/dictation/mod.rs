//! Dictation: the state machine and the controller thread that drives it.

mod controller;
mod state;

pub use controller::Dictation;
pub use state::{Input, Phase, TakeId};
