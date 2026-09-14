//! What the app needs from the outside world.

use std::sync::{Arc, atomic::AtomicBool};

use crate::domain::input::Input;

/// Shows the replay until `stop` is set, or until `dismisses` says an input
/// ends it.
pub trait Player {
    /// # Errors
    ///
    /// If the replay can't be shown.
    fn play(
        self,
        stop: Arc<AtomicBool>,
        dismisses: &mut dyn FnMut(Input) -> bool,
    ) -> Result<(), String>;
}

/// Whether the screensaver should keep running: one of its windows has focus
/// and the session isn't locked.
pub trait Presence {
    fn should_stay(&mut self) -> bool;
}

impl<F: FnMut() -> bool> Presence for F {
    fn should_stay(&mut self) -> bool {
        self()
    }
}
