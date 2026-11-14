//! Ultrasonic atomizer controller.
//!
//! Wiring (see readme.md in this folder):
//! - GPIO26 -> MOSFET gate, switches the atomizer board's power (10k pull-down keeps it off at boot)
//! - GPIO27 -> optocoupler LED, "presses" the board's trigger button
//! - GPIO19 <- mode button to GND (internal pull-up)
//! - GPIO18 <- water float switch to GND (internal pull-up), closed = water present

use std::thread;
use std::time::Duration;

use atomizer_logic::{Controller, Timing};
use debounce::{ButtonEvent, Debouncer};
use esp_idf_svc::hal::{
    gpio::{PinDriver, Pull},
    peripherals::Peripherals,
};

const FRAME_INTERVAL: Duration = Duration::from_millis(10);

/// Trigger timing in 10ms frames. Tune against your board if presses get missed.
const TIMING: Timing = Timing {
    press_frames: 15,  // 150ms press
    gap_frames: 40,    // 400ms between presses
    settle_frames: 50, // 500ms for the board to boot after power-on
};

const BUTTON_DEBOUNCE_FRAMES: u8 = 3;
/// Water level must read low for 0.5s before it counts, so sloshing doesn't trip it.
const WATER_DEBOUNCE_FRAMES: u8 = 50;

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;

    let mut power = PinDriver::output(pins.gpio26)?;
    let mut trigger = PinDriver::output(pins.gpio27)?;
    power.set_low()?;
    trigger.set_low()?;

    let mode_pin = PinDriver::input(pins.gpio19, Pull::Up)?;
    let water_pin = PinDriver::input(pins.gpio18, Pull::Up)?;

    let mut mode_button = Debouncer::new(BUTTON_DEBOUNCE_FRAMES);
    // "Pressed" here means the float switch is open: water is low
    let mut water_low = Debouncer::new(WATER_DEBOUNCE_FRAMES);

    let mut controller = Controller::new(TIMING);
    let mut last_status = controller.status();
    log::info!("Atomizer ready: {last_status:?}");

    loop {
        match water_low.update(water_pin.is_high()) {
            Some(ButtonEvent::Pressed) => log::warn!("Water: LOW"),
            Some(ButtonEvent::Released) => log::info!("Water: OK"),
            None => {}
        }
        let water_ok = !water_low.is_pressed();

        if mode_button.update(mode_pin.is_low()) == Some(ButtonEvent::Pressed) {
            controller.button_pressed(water_ok);
        }

        let outputs = controller.tick(water_ok);
        power.set_level(outputs.power.into())?;
        trigger.set_level(outputs.trigger.into())?;

        let status = controller.status();
        if status != last_status {
            if status.fault && !last_status.fault {
                log::warn!("Fault: water low, atomizer off. Refill, then press the button.");
            } else if !status.fault && last_status.fault {
                log::info!("Fault cleared");
            }
            if status.target != last_status.target {
                log::info!("Target: {}", status.target);
            }
            if status.board != last_status.board {
                log::info!("Board: {}", status.board);
            }
            last_status = status;
        }

        // Sleeping yields to the scheduler so the watchdog won't trigger
        thread::sleep(FRAME_INTERVAL);
    }
}
