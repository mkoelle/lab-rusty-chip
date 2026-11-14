use std::thread;
use std::time::Duration;

use esp_idf_svc::hal::{gpio::PinDriver, peripherals::Peripherals};

const BLINK_INTERVAL: Duration = Duration::from_millis(500);

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello, world!");

    let peripherals = Peripherals::take()?;
    let mut led = PinDriver::output(peripherals.pins.gpio33)?;

    loop {
        led.toggle()?;
        log::info!("LED is {}", if led.is_set_high() { "ON" } else { "OFF" });

        // Sleeping yields to the scheduler so the watchdog won't trigger
        thread::sleep(BLINK_INTERVAL);
    }
}
