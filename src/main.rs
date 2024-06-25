#![no_std]
#![no_main]
extern crate alloc;
extern crate core;

mod lamp;
mod ui;

use esp_backtrace as _;
use esp_hal::{
    clock::ClockControl,
    clock::Clocks,
    gpio::{self, Io, Level, Output},
    peripherals::Peripherals,
    prelude::*,
    rmt::Rmt,
    spi::{master::Spi, SpiMode},
    system::SystemControl,
    timer::systimer::SystemTimer,
};

use embassy_executor::Spawner;
use esp_hal_embassy;

use embedded_hal_bus::spi::ExclusiveDevice;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;

use embassy_time::{Duration, Ticker};

type ClocksType = Mutex<CriticalSectionRawMutex, Option<Clocks<'static>>>;
static CLOCKS: ClocksType = Mutex::new(None);

static CHANNEL: Channel<CriticalSectionRawMutex, u8, 64> = Channel::new();

#[embassy_executor::task]
pub async fn simu() {
    let mut hue: u8 = 0;
    let mut ticker = Ticker::every(Duration::from_millis(20));
    loop {
        if hue < 255 {
            hue = hue + 1;
        } else {
            hue = 0
        }

        CHANNEL.send(hue).await;
        ticker.next().await;
    }
}

#[main]
async fn main(spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);

    let clocks = ClockControl::max(system.clock_control).freeze();
    let systimer = SystemTimer::new_async(peripherals.SYSTIMER);

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let lcd_sclk = io.pins.gpio6;
    let lcd_mosi = io.pins.gpio7;
    let lcd_cs = Output::new(io.pins.gpio10, Level::High);

    let lcd_dc = Output::new(io.pins.gpio3, Level::High);
    let lcd_rst = Output::new(io.pins.gpio1, Level::High);
    let _lcd_bl = Output::new(io.pins.gpio2, Level::High);

    let led_pin = io.pins.gpio8;

    let spi = Spi::new(peripherals.SPI2, 80.MHz(), SpiMode::Mode0, &clocks).with_pins(
        Some(lcd_sclk),
        Some(lcd_mosi),
        gpio::NO_PIN,
        gpio::NO_PIN,
    );

    let spi_dev = ExclusiveDevice::new_no_delay(spi, lcd_cs).unwrap();

    let rmt = Rmt::new(peripherals.RMT, 80.MHz(), &clocks, None).unwrap();

    esp_println::logger::init_logger_from_env();

    esp_hal_embassy::init(&clocks, systimer);

    {
        *(CLOCKS.lock().await) = Some(clocks);
    }

    spawner.spawn(ui::run(spi_dev, lcd_dc, lcd_rst)).ok();
    spawner.spawn(lamp::run(rmt, led_pin)).ok();
    spawner.spawn(simu()).ok();
}
