//! Control logic for an ultrasonic atomizer (mist maker) board.
//!
//! The board has two inputs the ESP32 drives:
//! - **power**: switched by a MOSFET. Cutting power always returns the board to [`Mode::Off`].
//! - **trigger**: the board's push button, pressed through an optocoupler.
//!   Each press cycles `Off -> On -> Timed -> Off`.
//!
//! The board gives no feedback, so [`Controller`] tracks the board's mode by counting presses
//! from the known state after power-up. To turn off it cuts power instead of pressing,
//! which is instant and always correct.
//!
//! A low water level is a hard interlock: running the transducer dry destroys it.
//! The controller cuts power and latches a fault until water is back *and* the user presses
//! the button to acknowledge.
//!
//! Everything is measured in frames (calls to [`Controller::tick`]) so the logic has no clock.
#![cfg_attr(not(test), no_std)]

use core::fmt;

/// Atomizer board mode, in the order its push button cycles through them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Off,
    On,
    /// The board's second mode (interval or auto-off timer, depending on the board).
    Timed,
}

impl Mode {
    /// Mode after one button press.
    pub fn next(self) -> Self {
        match self {
            Mode::Off => Mode::On,
            Mode::On => Mode::Timed,
            Mode::Timed => Mode::Off,
        }
    }

    fn index(self) -> u8 {
        match self {
            Mode::Off => 0,
            Mode::On => 1,
            Mode::Timed => 2,
        }
    }

    /// Button presses needed to get from `self` to `target`.
    pub fn presses_to(self, target: Mode) -> u8 {
        (target.index() + 3 - self.index()) % 3
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Trigger pulse timing, in frames. Measure your board: too short a press or gap and it misses presses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// How long the trigger is held for one press.
    pub press_frames: u16,
    /// Pause after each press before the next one.
    pub gap_frames: u16,
    /// Wait after power-on before the first press, while the board boots.
    pub settle_frames: u16,
}

/// Pin levels to apply this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Outputs {
    /// Board power MOSFET on.
    pub power: bool,
    /// Trigger optocoupler on (button held down).
    pub trigger: bool,
}

/// Snapshot for logging and display. Compare with the previous one to detect changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    /// Mode the user asked for.
    pub target: Mode,
    /// Mode the board is believed to be in.
    pub board: Mode,
    /// Water ran low; stays set until acknowledged.
    pub fault: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Settling(u16),
    Pressing(u16),
    Gap(u16),
}

#[derive(Debug, Clone)]
pub struct Controller {
    timing: Timing,
    target: Mode,
    board: Mode,
    fault: bool,
    power: bool,
    phase: Phase,
}

impl Controller {
    pub const fn new(timing: Timing) -> Self {
        Self {
            timing,
            target: Mode::Off,
            board: Mode::Off,
            fault: false,
            power: false,
            phase: Phase::Idle,
        }
    }

    pub fn status(&self) -> Status {
        Status {
            target: self.target,
            board: self.board,
            fault: self.fault,
        }
    }

    /// Ask for a specific mode (e.g. from MQTT or a humidistat). Ignored while faulted.
    pub fn request(&mut self, mode: Mode) {
        if !self.fault {
            self.target = mode;
        }
    }

    /// User pressed the control button: acknowledges a fault if water is back,
    /// otherwise steps the target mode `Off -> On -> Timed -> Off`.
    pub fn button_pressed(&mut self, water_ok: bool) {
        if self.fault {
            if water_ok {
                self.fault = false;
            }
            return;
        }
        self.target = self.target.next();
    }

    /// Advance one frame. `water_ok` is the debounced water level.
    pub fn tick(&mut self, water_ok: bool) -> Outputs {
        if !water_ok {
            self.fault = true;
        }
        if self.fault {
            self.target = Mode::Off;
        }

        if self.target == Mode::Off {
            self.cut_power();
            return self.outputs();
        }

        if !self.power {
            // Power-up frame counts as the first settle frame
            self.power = true;
            self.board = Mode::Off;
            self.phase = Phase::Settling(self.timing.settle_frames);
            return self.outputs();
        }

        self.phase = match self.phase {
            Phase::Settling(n) if n > 1 => Phase::Settling(n - 1),
            Phase::Gap(n) if n > 1 => Phase::Gap(n - 1),
            Phase::Pressing(n) if n > 1 => Phase::Pressing(n - 1),
            Phase::Pressing(_) => {
                self.board = self.board.next();
                Phase::Gap(self.timing.gap_frames)
            }
            Phase::Settling(_) | Phase::Gap(_) | Phase::Idle => Phase::Idle,
        };

        if self.phase == Phase::Idle && self.board != self.target {
            self.phase = Phase::Pressing(self.timing.press_frames);
        }

        self.outputs()
    }

    fn cut_power(&mut self) {
        self.power = false;
        self.board = Mode::Off;
        self.phase = Phase::Idle;
    }

    fn outputs(&self) -> Outputs {
        Outputs {
            power: self.power,
            trigger: matches!(self.phase, Phase::Pressing(_)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMING: Timing = Timing {
        press_frames: 2,
        gap_frames: 3,
        settle_frames: 4,
    };

    /// Tick `frames` times, returning the outputs of each frame.
    fn run(c: &mut Controller, water_ok: bool, frames: usize) -> Vec<Outputs> {
        (0..frames).map(|_| c.tick(water_ok)).collect()
    }

    fn presses(outputs: &[Outputs]) -> usize {
        outputs
            .windows(2)
            .filter(|w| !w[0].trigger && w[1].trigger)
            .count()
            + usize::from(outputs.first().is_some_and(|o| o.trigger))
    }

    #[test]
    fn presses_to_counts_cycle() {
        assert_eq!(Mode::Off.presses_to(Mode::On), 1);
        assert_eq!(Mode::Off.presses_to(Mode::Timed), 2);
        assert_eq!(Mode::Timed.presses_to(Mode::On), 2);
        assert_eq!(Mode::On.presses_to(Mode::On), 0);
    }

    #[test]
    fn starts_off_with_power_cut() {
        let mut c = Controller::new(TIMING);
        assert_eq!(c.tick(true), Outputs::default());
    }

    #[test]
    fn turning_on_powers_up_settles_then_presses_once() {
        let mut c = Controller::new(TIMING);
        c.button_pressed(true);
        let out = run(&mut c, true, 20);
        assert!(out.iter().all(|o| o.power));
        assert!(
            out[..4].iter().all(|o| !o.trigger),
            "no press while settling"
        );
        assert_eq!(presses(&out), 1);
        assert_eq!(c.status().board, Mode::On);
    }

    #[test]
    fn press_lasts_press_frames() {
        let mut c = Controller::new(TIMING);
        c.request(Mode::On);
        let out = run(&mut c, true, 20);
        assert_eq!(out.iter().filter(|o| o.trigger).count(), 2);
    }

    #[test]
    fn timed_mode_needs_two_presses_with_gap() {
        let mut c = Controller::new(TIMING);
        c.request(Mode::Timed);
        let out = run(&mut c, true, 30);
        assert_eq!(presses(&out), 2);
        assert_eq!(c.status().board, Mode::Timed);
    }

    #[test]
    fn stepping_on_to_timed_presses_once_more() {
        let mut c = Controller::new(TIMING);
        c.request(Mode::On);
        run(&mut c, true, 20);
        c.button_pressed(true);
        assert_eq!(c.status().target, Mode::Timed);
        let out = run(&mut c, true, 20);
        assert_eq!(presses(&out), 1);
        assert_eq!(c.status().board, Mode::Timed);
    }

    #[test]
    fn off_cuts_power_immediately() {
        let mut c = Controller::new(TIMING);
        c.request(Mode::Timed);
        run(&mut c, true, 30);
        c.button_pressed(true);
        assert_eq!(c.tick(true), Outputs::default());
        assert_eq!(c.status().board, Mode::Off);
    }

    #[test]
    fn low_water_cuts_power_and_latches_fault() {
        let mut c = Controller::new(TIMING);
        c.request(Mode::On);
        run(&mut c, true, 20);
        assert_eq!(c.tick(false), Outputs::default());
        assert!(c.status().fault);

        // Water back is not enough on its own
        assert_eq!(c.tick(true), Outputs::default());
        c.request(Mode::On);
        assert_eq!(c.tick(true), Outputs::default());
        assert_eq!(c.status().target, Mode::Off);
    }

    #[test]
    fn fault_needs_water_before_acknowledge() {
        let mut c = Controller::new(TIMING);
        c.tick(false);
        c.button_pressed(false);
        assert!(c.status().fault, "acknowledge ignored while water is low");
        c.button_pressed(true);
        assert!(!c.status().fault);
        assert_eq!(c.status().target, Mode::Off, "acknowledge does not turn on");
    }

    #[test]
    fn water_loss_mid_press_releases_trigger() {
        let mut c = Controller::new(TIMING);
        c.request(Mode::On);
        let out = run(&mut c, true, 6);
        assert!(out.last().unwrap().trigger);
        assert_eq!(c.tick(false), Outputs::default());
    }
}
