use std::thread;
use std::time::Duration;

use anyhow::anyhow;
use esp_idf_svc::hal::{
    gpio::PinDriver,
    i2c::{I2cConfig, I2cDriver},
    peripherals::Peripherals,
    prelude::*,
};

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};

use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};

const FRAME_INTERVAL: Duration = Duration::from_millis(10);

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello, world!");

    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;

    let mut led = PinDriver::output(pins.gpio33)?;
    let mut led_two = PinDriver::output(pins.gpio2)?;

    let config = I2cConfig::new().baudrate(400u32.kHz().into());
    let i2c_driver = I2cDriver::new(peripherals.i2c0, pins.gpio21, pins.gpio22, &config)?;

    // display-interface errors don't implement std::error::Error, so map them by hand
    let interface = I2CDisplayInterface::new(i2c_driver);
    let mut display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display
        .init()
        .map_err(|e| anyhow!("Display init error: {e:?}"))?;

    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    let mut count: u32 = 0;

    loop {
        led.toggle()?;
        led_two.toggle()?;

        display.clear_buffer();
        Text::new("Hello World !!!", Point::new(10, 12), style)
            .draw(&mut display)
            .map_err(|e| anyhow!("Draw error: {e:?}"))?;
        Text::new(&count.to_string(), Point::new(10, 22), style)
            .draw(&mut display)
            .map_err(|e| anyhow!("Draw error: {e:?}"))?;
        display.flush().map_err(|e| anyhow!("Flush error: {e:?}"))?;

        count = count.wrapping_add(1);

        // Sleeping yields to the scheduler so the watchdog won't trigger
        thread::sleep(FRAME_INTERVAL);
    }
}
