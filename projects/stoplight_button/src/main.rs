use std::thread;
use std::time::Duration;

use esp_idf_svc::hal::{
    gpio::{Input, InputPin, PinDriver, Pull},
    peripherals::Peripherals,
};
use esp_idf_svc::sys::EspError;
use stoplight_logic::{ButtonEvent, Debouncer, Light, Stoplight};

const FRAME_INTERVAL: Duration = Duration::from_millis(10);
/// Number of frames each light stays on in auto mode (50 x 10ms = 500ms).
const FRAMES_PER_LIGHT: u32 = 50;
/// Readings a button must hold steady before a press/release counts (3 x 10ms = 30ms).
const DEBOUNCE_FRAMES: u8 = 3;

/// Active-low push button with a pull-up, debounced in software.
struct Button<'d> {
    name: &'static str,
    pin: PinDriver<'d, Input>,
    debouncer: Debouncer,
}

impl<'d> Button<'d> {
    fn new(name: &'static str, pin: impl InputPin + 'd) -> Result<Self, EspError> {
        Ok(Self {
            name,
            pin: PinDriver::input(pin, Pull::Up)?,
            debouncer: Debouncer::new(DEBOUNCE_FRAMES),
        })
    }

    /// Reads the pin once; returns `true` on a debounced press.
    fn just_pressed(&mut self) -> bool {
        match self.debouncer.update(self.pin.is_low()) {
            Some(ButtonEvent::Pressed) => {
                log::info!("{} button pressed", self.name);
                true
            }
            Some(ButtonEvent::Released) => {
                log::info!("{} button released", self.name);
                false
            }
            None => false,
        }
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

    let mut stoplight = Stoplight::new(FRAMES_PER_LIGHT);
    log::info!("Stoplight ready, light: {}", stoplight.light());

    loop {
        let changes = stoplight.tick(advance_button.just_pressed(), auto_button.just_pressed());

        if let Some(auto_mode) = changes.auto_mode {
            log::info!("Auto mode: {}", if auto_mode { "ON" } else { "OFF" });
        }
        if let Some(light) = changes.light {
            log::info!("Light: {light}");
        }

        let light = stoplight.light();
        red_light.set_level((light == Light::Red).into())?;
        green_light.set_level((light == Light::Green).into())?;
        yellow_light.set_level((light == Light::Yellow).into())?;

        // Sleeping yields to the scheduler so the watchdog won't trigger
        thread::sleep(FRAME_INTERVAL);
    }
}
