//! The Player port: gitlogue's UI, typing out random commits from one
//! repository.

use std::sync::{Arc, atomic::AtomicBool};

use gitlogue::{
    PlaybackOrder,
    crossterm::event::{Event, KeyEventKind, MouseEventKind},
    git::{CommitMetadata, GitRepository},
    theme::Theme,
    ui::{InputAction, UI},
};

use crate::{domain::input::Input, ports::Player};

/// Plays `first`, then random commits from `repo`.
pub struct Replay<'a> {
    pub repo: &'a GitRepository,
    pub first: CommitMetadata,
    pub theme: Theme,
    /// Milliseconds per typed character.
    pub speed_ms: u64,
}

impl Player for Replay<'_> {
    fn play(
        self,
        stop: Arc<AtomicBool>,
        dismisses: &mut dyn FnMut(Input) -> bool,
    ) -> Result<(), String> {
        let mut ui = UI::embedded(
            self.speed_ms,
            Some(self.repo),
            self.theme,
            PlaybackOrder::Random,
            Vec::new(),
            stop,
        );
        // Input either dismisses the screensaver or is dropped: gitlogue's
        // own keys (pause, menu, quit) have no place in a screensaver.
        ui.set_input_hook(|event| match input_of(event) {
            Some(input) if dismisses(input) => InputAction::Exit,
            _ => InputAction::Ignore,
        });
        ui.load_commit(self.first);
        ui.run().map_err(|error| format!("{error:#}"))
    }
}

/// What a terminal event means to the screensaver, if anything.
fn input_of(event: &Event) -> Option<Input> {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => Some(Input::Key),
        Event::Paste(_) => Some(Input::Key),
        Event::Mouse(mouse) => Some(match mouse.kind {
            MouseEventKind::Moved => Input::PointerMotion {
                column: mouse.column,
                row: mouse.row,
            },
            _ => Input::Pointer,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitlogue::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent};

    fn key(kind: KeyEventKind) -> Event {
        Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            kind,
        ))
    }

    fn mouse(kind: MouseEventKind) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column: 12,
            row: 7,
            modifiers: KeyModifiers::NONE,
        })
    }

    #[test]
    fn key_presses_and_paste_are_keys() {
        assert_eq!(input_of(&key(KeyEventKind::Press)), Some(Input::Key));
        assert_eq!(input_of(&key(KeyEventKind::Repeat)), Some(Input::Key));
        assert_eq!(input_of(&key(KeyEventKind::Release)), None);
        assert_eq!(input_of(&Event::Paste("x".to_owned())), Some(Input::Key));
    }

    #[test]
    fn the_pointer_moves_to_a_cell_or_clicks() {
        assert_eq!(
            input_of(&mouse(MouseEventKind::Moved)),
            Some(Input::PointerMotion { column: 12, row: 7 })
        );
        for kind in [
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::Up(MouseButton::Left),
            MouseEventKind::Drag(MouseButton::Left),
            MouseEventKind::ScrollDown,
        ] {
            assert_eq!(input_of(&mouse(kind)), Some(Input::Pointer), "{kind:?}");
        }
    }

    #[test]
    fn window_events_are_not_input() {
        for event in [Event::Resize(80, 24), Event::FocusGained, Event::FocusLost] {
            assert_eq!(input_of(&event), None, "{event:?}");
        }
    }
}
