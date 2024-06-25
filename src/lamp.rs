use esp_hal::{gpio::OutputPin, peripheral::Peripheral};
use esp_hal::{rmt::Rmt, Blocking};
use esp_hal_smartled::{smartLedBuffer, SmartLedsAdapter};

use smart_leds::{
    brightness, gamma,
    hsv::{hsv2rgb, Hsv},
    SmartLedsWrite,
};

use super::{CHANNEL, CLOCKS};

#[embassy_executor::task]
pub async fn run(rmt: Rmt<'static, Blocking>, pin: impl Peripheral<P: OutputPin> + 'static) {
    let mut color = Hsv {
        hue: 0,
        sat: 255,
        val: 255,
    };

    let mut data;
    let mut led;

    let rmt_buffer = smartLedBuffer!(1);

    {
        let mut clocks_unlocked = CLOCKS.lock().await;
        if let Some(clocks) = clocks_unlocked.as_mut() {
            led = SmartLedsAdapter::new(rmt.channel0, pin, rmt_buffer, &clocks);
        } else {
            panic!();
        }
    }

    loop {
        color.hue = CHANNEL.receive().await;
        data = [hsv2rgb(color)];
        led.write(brightness(gamma(data.iter().cloned()), 10))
            .unwrap();
    }
}
