//! Software debouncing for buttons and switches, shared by firmware projects.
#![cfg_attr(not(test), no_std)]

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
