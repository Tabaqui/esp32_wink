use defmt::info;
use embassy_executor::task;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Timer};
use minicbor::{Decode, Encode};

#[task]
pub async fn led_task(l_res: &'static Sender<'static, NoopRawMutex, Light, 3>) {
    // init_led smart_led

    // light led on

    // send turn on is done

    // let lights = [Light::new(1), Light::new(2), Light::new(3)];
    let lights = [Light::new_turned_off(0), Light::new_turned_off(1), Light::new_turned_off(2)];
    info!("Trying to send");

    for light in lights {
        l_res.send(light).await;
        Timer::after(Duration::from_secs(1)).await;
    }
    info!("Sent");
}

#[derive(defmt::Format)]
#[derive(Encode, Decode)]
#[cbor(map)]
pub struct Light {
    #[n(0)] state: bool,
    #[n(1)] num: u32,
}

impl Light {
    fn new_turned_off(num: u32) -> Self {
        Light { state: false, num }
    }
}
