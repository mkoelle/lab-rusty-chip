//! Hardware-independent logic for the stoplight project.
//!
//! Nothing in here touches GPIO, so it builds for the host and runs under `cargo test`.
//! The firmware feeds in raw button readings once per frame and applies the resulting [`Light`].
//!
//! `no_std` so the same crate works for both `esp-idf-svc` (std) and `esp-hal` (no_std) firmware.
#![cfg_attr(not(test), no_std)]

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    Red,
    Green,
    Yellow,
}

impl Light {
    pub fn next(self) -> Self {
        match self {
            Light::Red => Light::Green,
            Light::Green => Light::Yellow,
            Light::Yellow => Light::Red,
        }
    }
}

impl fmt::Display for Light {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonEvent {
    Pressed,
    Released,
}

/// Debounces a raw button reading and reports clean press/release edges.
///
/// A new state is accepted only after it has been read `stable_frames` times in a row,
/// which filters out contact bounce.
#[derive(Debug, Clone)]
pub struct Debouncer {
    stable_frames: u8,
    pressed: bool,
    candidate: bool,
    count: u8,
}

impl Debouncer {
    pub const fn new(stable_frames: u8) -> Self {
        Self {
            stable_frames,
            pressed: false,
            candidate: false,
            count: 0,
        }
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Feed one raw reading (`true` = button held down). Returns an event on a debounced edge.
    pub fn update(&mut self, is_down: bool) -> Option<ButtonEvent> {
        if is_down == self.pressed {
            self.count = 0;
            return None;
        }
        if is_down != self.candidate {
            self.candidate = is_down;
            self.count = 0;
        }
        self.count += 1;
        if self.count < self.stable_frames {
            return None;
        }
        self.pressed = is_down;
        self.count = 0;
        Some(if is_down {
            ButtonEvent::Pressed
        } else {
            ButtonEvent::Released
        })
    }
}

/// What changed during a [`Stoplight::tick`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Changes {
    /// New light, if it changed this frame.
    pub light: Option<Light>,
    /// New auto-mode setting, if it was toggled this frame.
    pub auto_mode: Option<bool>,
}

/// Stoplight controller: auto mode cycles every `frames_per_light` ticks,
/// manual mode advances only on the advance button.
#[derive(Debug, Clone)]
pub struct Stoplight {
    light: Light,
    auto_mode: bool,
    frames: u32,
    frames_per_light: u32,
}

impl Stoplight {
    pub const fn new(frames_per_light: u32) -> Self {
        Self {
            light: Light::Red,
            auto_mode: true,
            frames: 0,
            frames_per_light,
        }
    }

    pub fn light(&self) -> Light {
        self.light
    }

    pub fn auto_mode(&self) -> bool {
        self.auto_mode
    }

    /// Advance one frame. `advance_pressed` / `auto_pressed` are debounced press edges.
    pub fn tick(&mut self, advance_pressed: bool, auto_pressed: bool) -> Changes {
        let mut changes = Changes::default();

        if auto_pressed {
            self.auto_mode = !self.auto_mode;
            self.frames = 0;
            changes.auto_mode = Some(self.auto_mode);
        }

        let advance = if self.auto_mode {
            self.frames += 1;
            self.frames >= self.frames_per_light
        } else {
            advance_pressed
        };

        if advance {
            self.frames = 0;
            self.light = self.light.next();
            changes.light = Some(self.light);
        }

        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_cycles_red_green_yellow() {
        assert_eq!(Light::Red.next(), Light::Green);
        assert_eq!(Light::Green.next(), Light::Yellow);
        assert_eq!(Light::Yellow.next(), Light::Red);
    }

    #[test]
    fn light_display_matches_name() {
        assert_eq!(Light::Yellow.to_string(), "Yellow");
    }

    #[test]
    fn debouncer_needs_stable_reads() {
        let mut b = Debouncer::new(3);
        assert_eq!(b.update(true), None);
        assert_eq!(b.update(true), None);
        assert_eq!(b.update(true), Some(ButtonEvent::Pressed));
        assert!(b.is_pressed());
        assert_eq!(b.update(true), None, "held button reports only one edge");
    }

    #[test]
    fn debouncer_ignores_bounce() {
        let mut b = Debouncer::new(3);
        for reading in [true, false, true, false, true] {
            assert_eq!(b.update(reading), None);
        }
        assert!(!b.is_pressed());
        b.update(true);
        assert_eq!(b.update(true), Some(ButtonEvent::Pressed));
    }

    #[test]
    fn debouncer_reports_release() {
        let mut b = Debouncer::new(1);
        assert_eq!(b.update(true), Some(ButtonEvent::Pressed));
        assert_eq!(b.update(false), Some(ButtonEvent::Released));
    }

    #[test]
    fn auto_mode_advances_after_frames() {
        let mut s = Stoplight::new(3);
        assert_eq!(s.tick(false, false).light, None);
        assert_eq!(s.tick(false, false).light, None);
        assert_eq!(s.tick(false, false).light, Some(Light::Green));
        assert_eq!(s.light(), Light::Green);
    }

    #[test]
    fn advance_button_ignored_in_auto_mode() {
        let mut s = Stoplight::new(100);
        assert_eq!(s.tick(true, false).light, None);
        assert_eq!(s.light(), Light::Red);
    }

    #[test]
    fn manual_mode_advances_only_on_button() {
        let mut s = Stoplight::new(2);
        let changes = s.tick(false, true);
        assert_eq!(changes.auto_mode, Some(false));
        for _ in 0..10 {
            assert_eq!(s.tick(false, false).light, None);
        }
        assert_eq!(s.tick(true, false).light, Some(Light::Green));
    }

    #[test]
    fn re_enabling_auto_restarts_frame_count() {
        let mut s = Stoplight::new(3);
        s.tick(false, false);
        s.tick(false, false);
        s.tick(false, true); // auto off
        let changes = s.tick(false, true); // auto on, counts as frame 1
        assert_eq!(changes.auto_mode, Some(true));
        assert_eq!(changes.light, None);
        s.tick(false, false);
        assert_eq!(s.tick(false, false).light, Some(Light::Green));
    }
}
