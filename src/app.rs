//! One screensaver session: the replay, and a watch for focus loss.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use crate::{
    domain::{
        focus::FocusWatch,
        input::{Dismissal, StopReason},
    },
    ports::{Player, Presence},
};

/// Plays until an input dismisses the screensaver, `stop` is set (by a
/// signal), or, given a `presence` to watch, the screensaver loses focus.
/// `now` is the time since the session started; `sleep` pauses between
/// presence checks. Returns why it stopped, unless it was `stop`.
///
/// # Errors
///
/// If the player fails.
pub fn run<N, S>(
    player: impl Player,
    presence: Option<impl Presence + Send>,
    stop: &Arc<AtomicBool>,
    now: N,
    sleep: S,
) -> Result<Option<StopReason>, String>
where
    N: Fn() -> Duration + Sync,
    S: Fn() + Sync,
{
    let flag: &AtomicBool = stop;
    let (now, sleep) = (&now, &sleep);
    thread::scope(|scope| {
        let watcher =
            presence.map(|presence| scope.spawn(move || watch(presence, flag, now, sleep)));
        let mut dismissal = Dismissal::default();
        let mut reason = None;
        let outcome = player.play(Arc::clone(stop), &mut |input| {
            reason = reason.or_else(|| dismissal.judge(input, now()));
            reason.is_some()
        });
        // Ends the watch, if the replay ended some other way.
        flag.store(true, Ordering::Relaxed);
        let focus_left = watcher.is_some_and(|watcher| watcher.join().unwrap_or(false));
        outcome.map(|()| {
            if focus_left {
                Some(StopReason::FocusLeft)
            } else {
                reason
            }
        })
    })
}

/// Asks `presence` whether the screensaver should stay, with a `sleep`
/// between asks, until `stop` is set. Sets `stop` itself once the screensaver
/// has been absent for the grace period, and then returns true.
fn watch(
    mut presence: impl Presence,
    stop: &AtomicBool,
    now: impl Fn() -> Duration,
    sleep: impl Fn(),
) -> bool {
    let mut focus = FocusWatch::default();
    while !stop.load(Ordering::Relaxed) {
        if focus.observe(presence.should_stay(), now()) {
            stop.store(true, Ordering::Relaxed);
            return true;
        }
        sleep();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{focus::GRACE, input::Input};
    use std::{
        sync::atomic::AtomicU64,
        time::{Duration, Instant},
    };

    /// A clock that only moves when someone sleeps.
    struct Clock(AtomicU64);

    impl Clock {
        fn at(ms: u64) -> Self {
            Self(AtomicU64::new(ms))
        }

        fn now(&self) -> Duration {
            Duration::from_millis(self.0.load(Ordering::Relaxed))
        }

        /// Moves on 100 ms. A watch still going after two minutes never
        /// will: fail rather than hang.
        fn sleep(&self) {
            let now = self.0.fetch_add(100, Ordering::Relaxed) + 100;
            assert!(now <= 120_000, "still watching after two minutes");
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Feeds its inputs to the session, then plays on until stopped, the way
    /// gitlogue's UI would.
    struct FakePlayer {
        inputs: Vec<Input>,
        fails: bool,
    }

    impl FakePlayer {
        fn with(inputs: &[Input]) -> Self {
            Self {
                inputs: inputs.to_vec(),
                fails: false,
            }
        }
    }

    impl Player for FakePlayer {
        fn play(
            self,
            stop: Arc<AtomicBool>,
            dismisses: &mut dyn FnMut(Input) -> bool,
        ) -> Result<(), String> {
            if self.fails {
                return Err("no terminal".to_owned());
            }
            for input in self.inputs {
                if dismisses(input) {
                    return Ok(());
                }
            }
            // An error rather than a panic, so the session still stops the
            // watch and the test fails instead of hanging.
            let deadline = Instant::now() + Duration::from_secs(3);
            while !stop.load(Ordering::Relaxed) {
                if Instant::now() > deadline {
                    return Err("never stopped".to_owned());
                }
                thread::sleep(Duration::from_millis(1));
            }
            Ok(())
        }
    }

    const NO_PRESENCE: Option<fn() -> bool> = None;

    #[test]
    fn an_input_that_dismisses_ends_the_session_and_the_watch() {
        let clock = Clock::at(10_000);
        let stop = Arc::new(AtomicBool::new(false));
        let reason = run(
            FakePlayer::with(&[Input::Key]),
            Some(|| true),
            &stop,
            || clock.now(),
            || clock.sleep(),
        );
        assert_eq!(reason, Ok(Some(StopReason::KeyPress)));
        assert!(stop.load(Ordering::Relaxed));
    }

    #[test]
    fn inputs_are_judged_by_the_time_since_the_start() {
        let clock = Clock::at(0);
        let stop = Arc::new(AtomicBool::new(false));
        // Too early to count; the watch then finds focus gone.
        let reason = run(
            FakePlayer::with(&[Input::Key, Input::Pointer]),
            Some(|| false),
            &stop,
            || clock.now(),
            || clock.sleep(),
        );
        assert_eq!(reason, Ok(Some(StopReason::FocusLeft)));
        assert!(clock.now() >= GRACE);
    }

    #[test]
    fn the_first_dismissal_is_the_reason() {
        /// Keeps feeding input after being told to stop.
        struct Stubborn;
        impl Player for Stubborn {
            fn play(
                self,
                _: Arc<AtomicBool>,
                dismisses: &mut dyn FnMut(Input) -> bool,
            ) -> Result<(), String> {
                assert!(dismisses(Input::Pointer));
                assert!(dismisses(Input::Key));
                Ok(())
            }
        }

        let clock = Clock::at(10_000);
        let stop = Arc::new(AtomicBool::new(false));
        let reason = run(
            Stubborn,
            NO_PRESENCE,
            &stop,
            || clock.now(),
            || clock.sleep(),
        );
        assert_eq!(reason, Ok(Some(StopReason::ClickOrScroll)));
    }

    #[test]
    fn a_signal_stops_it_without_a_reason() {
        let clock = Clock::at(0);
        let stop = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&stop);
        let reason = thread::scope(|scope| {
            scope.spawn(move || {
                thread::sleep(Duration::from_millis(20));
                signal.store(true, Ordering::Relaxed);
            });
            run(
                FakePlayer::with(&[]),
                Some(|| true),
                &stop,
                || clock.now(),
                || clock.sleep(),
            )
        });
        assert_eq!(reason, Ok(None));
    }

    #[test]
    fn a_failing_player_still_ends_the_watch() {
        let clock = Clock::at(0);
        let stop = Arc::new(AtomicBool::new(false));
        let player = FakePlayer {
            inputs: vec![],
            fails: true,
        };
        let reason = run(
            player,
            Some(|| true),
            &stop,
            || clock.now(),
            || clock.sleep(),
        );
        assert_eq!(reason, Err("no terminal".to_owned()));
        assert!(stop.load(Ordering::Relaxed));
    }

    #[test]
    fn the_watch_stops_it_after_the_grace_period_only() {
        let clock = Clock::at(0);
        let stop = AtomicBool::new(false);
        let mut asked = 0;
        let absent = || {
            asked += 1;
            false
        };
        assert!(watch(absent, &stop, || clock.now(), || clock.sleep()));
        assert!(stop.load(Ordering::Relaxed));
        assert_eq!(clock.now(), GRACE);
        assert_eq!(asked, 16, "every 100 ms from 0 to 1500 ms");
    }

    #[test]
    fn the_watch_ends_quietly_once_stopped() {
        let clock = Clock::at(0);
        let stop = AtomicBool::new(false);
        let mut asked = 0;
        let present = || {
            asked += 1;
            true
        };
        let sleep = || {
            clock.sleep();
            if clock.now() >= Duration::from_secs(60) {
                stop.store(true, Ordering::Relaxed);
            }
        };
        assert!(!watch(present, &stop, || clock.now(), sleep));
        assert_eq!(asked, 600);

        let never = || -> bool { unreachable!("stopped before asking") };
        assert!(!watch(never, &stop, || clock.now(), || clock.sleep()));
    }
}
