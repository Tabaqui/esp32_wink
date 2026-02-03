#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
mod mqtt;
mod led;

use alloc::boxed::Box;
use defmt::error;
use defmt::info;
use embassy_executor::Spawner;
use embassy_net::DhcpConfig;
use embassy_net::{
    Runner, StackResources,
};
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::{Duration, Timer};
use esp_hal::peripherals;
use esp_hal::{clock::CpuClock, rng::Rng, timer::timg::TimerGroup};
use esp_radio::wifi::{
    ClientConfig, ModeConfig, ScanConfig, WifiController, WifiDevice, WifiEvent, WifiStaState,
};

use crate::led::Light;
use crate::led::Ready;

use {esp_backtrace as _, esp_println as _};
extern crate alloc;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.0.1

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals: esp_hal::peripherals::Peripherals = esp_hal::init(config);
    // let rmt = ;

    esp_alloc::heap_allocator!(size: 128 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);



    info!("Embassy initialized!");

    let cr = Box::leak(Box::new(esp_radio::init().unwrap()));
    let (mut controller, interfaces) =
        esp_radio::wifi::new(cr, peripherals.WIFI, Default::default()).unwrap();

    controller
        .set_power_saving(esp_radio::wifi::PowerSaveMode::Maximum)
        .unwrap();

    let wifi_interface = interfaces.sta;

    let config = embassy_net::Config::dhcpv4(DhcpConfig::default());

    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    // Init network stack
    let (stack, runner) = embassy_net::new(
        wifi_interface,
        config,
        Box::leak(Box::new(StackResources::<3>::new())),
        seed,
    );

    // let l_channel = Channel::<NoopRawMutex, u32, 3>::new();

    let led_channel = Box::leak(Box::new(Channel::<NoopRawMutex, Light, 3>::new()));

    let led_receiver = Box::leak(Box::new(led_channel.receiver()));
    let led_sender = Box::leak(Box::new(led_channel.sender()));

    
let config = esp_hal::gpio::OutputConfig::default();
let rmt_pin: esp_hal::gpio::Output<'_> = esp_hal::gpio::Output::new(
    peripherals.GPIO8, 
    esp_hal::gpio::Level::High, 
    config
);

let led_status = Box::leak(Box::new(Channel::<NoopRawMutex, Ready, 3>::new()));
    
    spawner.spawn(connection(controller)).ok();
    spawner.spawn(net_task(runner)).ok();
    spawner.spawn(mqtt::mqtt_task(stack, led_sender, led_status)).ok();
    spawner.spawn(led::led_task(led_receiver, peripherals.RMT, rmt_pin, led_status)).ok();

    

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    info!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            info!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_secs(5)).await;
    }

    loop {
        info!("Main got waiting");
        Timer::after_secs(10).await;
    }
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    info!("start connection task");
    info!("Device capabilities: {:?}", controller.capabilities());
    loop {
        if esp_radio::wifi::sta_state() == WifiStaState::Connected {
            // wait until we're no longer connected
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let station_config = ModeConfig::Client(
                ClientConfig::default()
                    .with_ssid(SSID.into())
                    .with_password(PASSWORD.into()),
            );
            controller.set_config(&station_config).unwrap();
            info!("Starting wifi");
            controller.start_async().await.unwrap();
            info!("Wifi started!");

            info!("Scan");
            let scan_config = ScanConfig::default().with_max(5);
            let result = controller
                .scan_with_config_async(scan_config)
                .await
                .unwrap();
            for ap in result {
                info!("{:?}", ap);
            }
        }
        info!("About to connect...");

        match controller.connect_async().await {
            Ok(_) => info!("Wifi connected!"),
            Err(e) => {
                error!("Failed to connect to wifi: {:?}", e);
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}
