use core::{error::Error, iter::Map};

use alloc::{boxed::Box, vec::Vec};
use defmt::info;
use embassy_executor::task;
use embassy_futures::{join::join, select::select};
use embassy_sync::{
    blocking_mutex::raw::NoopRawMutex,
    channel::{Channel, Receiver},
};
use embassy_time::{Duration, Timer, WithTimeout};
use esp_hal::{
    gpio::Output,
    peripherals::{GPIO8, RMT},
    rmt::Rmt,
};
use esp_hal_smartled::{
    RmtSmartLeds, Ws2812Timing, Ws2812bTiming, buffer_size,
    color_order::{self, Rgb},
};
use minicbor::{Decode, Encode};
use smart_leds::{
    RGB, SmartLedsWriteAsync, brightness, gamma,
    hsv::{Hsv, hsv2rgb},
};
use smart_leds_trait::RGB8;
use static_cell::StaticCell;

static CH: StaticCell<Channel<NoopRawMutex, Ready, 3>> = StaticCell::new();

const LEDS: usize = 50;

pub struct StripMisc<'a> {
    rmt: RMT<'a>,
    gpio: Output<'a>,
    ch: &'a mut Channel<NoopRawMutex, Ready, 3>,
}

impl<'a> StripMisc<'a> {
    pub fn new(rmt: RMT<'a>, gpio: Output<'a>) -> Result<Self, &'a str> {
        
        let o_ch = CH.try_init(Channel::new());
        match o_ch {
            Some(ch) => Ok(StripMisc { rmt, gpio, ch }),
            None => Err("was taken away!"),
        }
    }       
}

pub fn init<'a>(
    s_m: StripMisc<'a>,
) -> (
    &'a mut Channel<NoopRawMutex, Ready, 3>,
    RmtSmartLeds<
        'a,
        1201,
        esp_hal::Async,
        smart_leds_trait::RGB<u8>,
        color_order::Grb,
        Ws2812bTiming,
    >,
) {
    let led: RmtSmartLeds<_, _, _, _, _> = {
        let freq = esp_hal::time::Rate::from_mhz(80);

        let rmt = Rmt::new(s_m.rmt, freq)
            .expect("Failed to initialize RMT0")
            .into_async();
        RmtSmartLeds::<{ buffer_size::<RGB8>(LEDS) }, _, RGB8, color_order::Grb, Ws2812bTiming>::new(
            rmt.channel0,
            s_m.gpio,
        )
        .unwrap()
    };

    (s_m.ch, led)
}

fn get_red(val: u8) -> Hsv {
    Hsv {
        hue: 0,
        sat: 255,
        val,
    }
}

#[task]
pub async fn receive_light(
    ch: &'static Channel<NoopRawMutex, Ready, 3>,
    mut smart_leds: RmtSmartLeds<
        'static,
        1201,
        esp_hal::Async,
        smart_leds_trait::RGB<u8>,
        color_order::Grb,
        Ws2812bTiming,
    >,
) {
    loop {
        let n_ready = ch.receive().await;

        let enlight = n_ready.enlight + n_ready.blink;
        let colors = [0; LEDS]
            .iter()
            .map(|val| if val < &enlight { 255u8 } else { 0u8 })
            .map(|val| { get_red(val) })
            .map(hsv2rgb);

        let g = gamma(colors);
        let b = brightness(g, 10);
        let fut = smart_leds.write(b);

        let (_, res) = join(Timer::after_millis(500), fut).await;
        res.unwrap();

        info!(" Don ")
        // smart_leds.write(iterator);
    }
}

#[task]
pub async fn led_task(
    // l_res: &'static Sender<'static, NoopRawMutex, Light, 3>,
    l_rec: &'static Receiver<'static, NoopRawMutex, Light, 3>,
    rmt: RMT<'static>,
    gpio8: Output<'static>,
    led_status_channel: &'static Channel<NoopRawMutex, Ready, 3>,
) {
    let freq = esp_hal::time::Rate::from_mhz(80);

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

    let color = Hsv {
        hue: 0,
        sat: 255,
        val: 255,
    };
    let mut data;

    loop {
        let r = l_rec.receive();
        let c = led_status_channel.receive();

        let s = select(r, c).await;

        data = [hsv2rgb(color); LEDS];

        match s {
            embassy_futures::select::Either::First(l) => {
                let fut = led.write(brightness(gamma(data.iter().cloned()), 128));
                let f = fut.await;
                f.unwrap();
                Timer::after_secs(2).await;
            }
            embassy_futures::select::Either::Second(r) => {
                for i in 0..100 {
                    info!("Blink");

                    let val: Box<dyn Fn(usize) -> u8> =
                        Box::new(|t: usize| if t < r.blink as usize { 255 } else { 0 });
                    let strip_loading = (0..LEDS)
                        .map(val)
                        .map(|val| Hsv {
                            hue: 0,
                            sat: 255,
                            val,
                        })
                        .map(hsv2rgb);

                    let fut = led.write(brightness(gamma(strip_loading), 128));
                    let f = fut.await;
                    f.unwrap();
                    Timer::after_millis(200).await;
                }
            }
        }

        // // loop {
        // // Iterate over the rainbow!
        // for val in 0..=255 {
        //     // color.val = val;
        //     // Convert from the HSV color space (where we can easily transition from one
        //     // color to the other) to the RGB color space that we can then send to the LED
        //     data = [hsv2rgb(color); LEDS];
        //     // When sending to the LED, we do a gamma correction first (see smart_leds
        //     // documentation for details) and then limit the brightness to 10 out of 255 so
        //     // that the output it's not too bright.

        //     // This call already prepares the buffer.

        //     let dic = data.iter().cloned();
        //     let fut = led.write(brightness(gamma(data.iter().cloned()), val));
        //     // Put more led.write() calls (for other drivers) and other peripheral preparations here...

        //     // Dispatch all the LED writes at once.
        //     // (We simulate the second write instead with a delay.)
        //     let (_, res) = join(Timer::after_millis(20), fut).await;
        //     res.unwrap();
        //     info!("Enlighten")
        // }
    }
}

// trait Led {
//     events: Led
// }

#[derive(defmt::Format, Encode, Decode)]
#[cbor(map)]
pub struct Light {
    #[n(0)]
    on: bool,
    #[n(1)]
    num: i16,
}

impl Light {
    pub fn get_off(num: i16) -> Self {
        Light { on: false, num }
    }
    pub fn get_on(num: i16) -> Self {
        Light { on: true, num }
    }
}

pub struct Ready {
    enlight: u8,
    blink: u8,
    blink_wait: Duration,
}

impl Ready {
    pub fn ip() -> Self {
        Ready {
            enlight: (LEDS / 4) as u8,
            blink: (LEDS / 4) as u8,
            blink_wait: Duration::from_millis(100),
        }
    }

    pub fn tcp() -> Self {
        Ready {
            enlight: (LEDS / 2) as u8,
            blink: (LEDS / 4) as u8,
            blink_wait: Duration::from_millis(100),
        }
    }
}
