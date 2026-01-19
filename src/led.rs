use defmt::info;
use embassy_executor::task;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Timer};
use serde::{Deserialize, Serialize};

#[task]
pub async fn led_task(l_res: Sender<'static, NoopRawMutex, Light, 3>) {
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
#[derive(Deserialize, Serialize)]
pub struct Light {
    state: bool,
    num: u8,
}

impl Light {
    fn new_turned_off(num: u8) -> Self {
        Light { state: false, num }
    }
}
