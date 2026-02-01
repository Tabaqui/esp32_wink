
use defmt::info;
use embassy_executor::task;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Receiver};
use embassy_time::{Duration, Timer, WithTimeout};
use esp_hal::{
    gpio::Output,
    peripherals::{GPIO8, RMT},
    rmt::Rmt,
};
use embassy_futures::join::join;
use esp_hal_smartled::{RmtSmartLeds, Ws2812Timing, Ws2812bTiming, buffer_size, color_order};
use minicbor::{Decode, Encode};
use smart_leds::{SmartLedsWriteAsync, brightness, gamma, hsv::{Hsv, hsv2rgb}};
use smart_leds_trait::RGB8;

#[task]
pub async fn led_task(
    // l_res: &'static Sender<'static, NoopRawMutex, Light, 3>,
    l_rec: &'static Receiver<'static, NoopRawMutex, Light, 3>,
    rmt: RMT<'static>,
    gpio8: Output<'static>,
) {
    let freq = esp_hal::time::Rate::from_mhz(80);

    const LEDS: usize = 50;

    let mut led: RmtSmartLeds<
        '_,
        _,
        esp_hal::Async,
        smart_leds_trait::RGB<u8>,
        color_order::Grb,
        Ws2812bTiming,
    > = {
        let rmt = Rmt::new(rmt, freq)
            .expect("Failed to initialize RMT0")
            .into_async();
        RmtSmartLeds::<{ buffer_size::<RGB8>(LEDS) }, _, RGB8, color_order::Grb, Ws2812bTiming>::new(
            rmt.channel0,
            gpio8,
        )
        .unwrap()
    };

    let mut color = Hsv {
        hue: 0,
        sat: 255,
        val: 255,
    };
    let mut data;

    for n in 1..10 {

        let r = l_rec.receive().await;
        // if r.is_err() {
        //     info!("Timeout");
        //     break;
        // }
        
    }

    let mut sing = Hsv {
        hue: 0,
        sat: 255,
        val: 255,
    };

    data = [hsv2rgb(color); LEDS];
                let fut = led.write(brightness(gamma(data.iter().cloned()), 128));
    let f = fut.await;
    f.unwrap();
    Timer::after_millis(20).await;
    let fut = led.write(brightness(gamma(data.iter().cloned()), 0));
    let f = fut.await;
    f.unwrap();
    Timer::after_secs(1).await;


    loop {
        // Iterate over the rainbow!
        for val in 0..=255 {
            // color.val = val;
            // Convert from the HSV color space (where we can easily transition from one
            // color to the other) to the RGB color space that we can then send to the LED
            data = [hsv2rgb(color); LEDS];
            // When sending to the LED, we do a gamma correction first (see smart_leds
            // documentation for details) and then limit the brightness to 10 out of 255 so
            // that the output it's not too bright.

            // This call already prepares the buffer.
            let fut = led.write(brightness(gamma(data.iter().cloned()), val));
            // Put more led.write() calls (for other drivers) and other peripheral preparations here...

            // Dispatch all the LED writes at once.
            // (We simulate the second write instead with a delay.)
            let (_, res) = join(Timer::after_millis(20), fut).await;
            res.unwrap();
            info!("Enlighten")
        }
    }
}

#[derive(defmt::Format, Encode, Decode)]
#[cbor(map)]
pub struct Light {
    #[n(0)]
    on: bool,
    #[n(1)]
    num: i16,
}

impl Light {
    fn new_turned_off(num: i16) -> Self {
        Light { on: false, num }
    }
}
