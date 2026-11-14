use std::fmt;
use std::thread;
use std::time::Duration;

use esp_idf_svc::hal::{
    gpio::{Input, InputPin, OutputPin, PinDriver, Pull},
    peripheral::Peripheral,
    peripherals::Peripherals,
};
use esp_idf_svc::sys::EspError;

const FRAME_INTERVAL: Duration = Duration::from_millis(10);
/// Number of frames each light stays on in auto mode.
const FRAMES_PER_LIGHT: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Light {
    Red,
    Green,
    Yellow,
}

impl Light {
    fn next(self) -> Self {
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

/// Active-low push button with edge detection.
struct Button<'d, P: InputPin> {
    name: &'static str,
    pin: PinDriver<'d, P, Input>,
    pressed: bool,
}

impl<'d, P: InputPin + OutputPin> Button<'d, P> {
    fn new(name: &'static str, pin: impl Peripheral<P = P> + 'd) -> Result<Self, EspError> {
        let mut pin = PinDriver::input(pin)?;
        pin.set_pull(Pull::Up)?;
        Ok(Self {
            name,
            pin,
            pressed: false,
        })
    }

    /// Returns `true` once per press, on the falling edge.
    fn just_pressed(&mut self) -> bool {
        let is_down = self.pin.is_low();
        let edge = is_down && !self.pressed;
        if is_down != self.pressed {
            log::info!(
                "{} button {}",
                self.name,
                if is_down { "pressed" } else { "released" }
            );
            self.pressed = is_down;
        }
        edge
    }
}

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;

    let mut red_light = PinDriver::output(pins.gpio25)?;
    let mut green_light = PinDriver::output(pins.gpio33)?;
    let mut yellow_light = PinDriver::output(pins.gpio32)?;

    let mut advance_button = Button::new("Advance", pins.gpio5)?;
    let mut auto_button = Button::new("Auto", pins.gpio18)?;

    let mut frames = 0;
    let mut auto_mode = true;
    let mut light = Light::Red;

    loop {
        // Advance button steps through the lights, only in manual mode
        if advance_button.just_pressed() && !auto_mode {
            light = light.next();
            log::info!("Manual advance to: {light}");
        }

        // Auto button toggles automatic cycling
        if auto_button.just_pressed() {
            auto_mode = !auto_mode;
            log::info!("Auto mode: {}", if auto_mode { "ON" } else { "OFF" });
            frames = 0;
        }

        if auto_mode {
            frames += 1;
            if frames >= FRAMES_PER_LIGHT {
                frames = 0;
                light = light.next();
            }
        }

        red_light.set_level((light == Light::Red).into())?;
        green_light.set_level((light == Light::Green).into())?;
        yellow_light.set_level((light == Light::Yellow).into())?;

        // Sleeping yields to the scheduler so the watchdog won't trigger
        thread::sleep(FRAME_INTERVAL);
    }
}
