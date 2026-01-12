#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
mod mqtt;

// use embedded_nal_async::Dns;

use alloc::boxed::Box;
use defmt::error;
use defmt::info;
use embassy_executor::Spawner;
use embassy_net::DhcpConfig;
use embassy_net::{
    Runner, Stack, StackResources,
    dns::DnsSocket,
    tcp::TcpSocket,
    tcp::client::{TcpClient, TcpClientState},
};
use embassy_time::{Duration, Timer};
use esp_hal::riscv::asm::nop;
use esp_hal::{clock::CpuClock, ram, rng::Rng, timer::timg::TimerGroup};
use esp_radio::wifi::{
    ClientConfig, ModeConfig, ScanConfig, WifiController, WifiDevice, WifiEvent, WifiStaState,
};
use static_cell::StaticCell;
use {esp_backtrace as _, esp_println as _};
// use reqwless::client::{HttpClient, TlsConfig};
extern crate alloc;

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.0.1

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 128 * 1024);
    // esp_alloc::heap_allocator!(#[ram(reclaimed)] size: 64 * 1024);
    // esp_alloc::heap_allocator!(size: 36 * 1024);
    // esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 64 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("Embassy initialized!");

    let cr = mk_static!(esp_radio::Controller<'static>, esp_radio::init().unwrap());
    let (mut controller, interfaces) =
        esp_radio::wifi::new(&*cr, peripherals.WIFI, Default::default()).unwrap();

    controller
        .set_power_saving(esp_radio::wifi::PowerSaveMode::Maximum)
        .unwrap();
    let wifi_interface = interfaces.sta;

    let config = embassy_net::Config::dhcpv4(DhcpConfig::default());

    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;
    let tls_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

    // Init network stack
    let (stack, runner) = embassy_net::new(
        wifi_interface,
        config,
        mk_static!(StackResources<5>, StackResources::<5>::new()),
        seed,
    );


    spawner.spawn(connection(controller)).ok();
    spawner.spawn(net_task(runner)).ok();

    let rx_buffer =  mk_static!([u8; 4096], [0; 4096]);
    let tx_buffer = Box::leak(Box::new([0; 4096]));

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
        Timer::after(Duration::from_millis(500)).await;
    }

    // loop {
    Timer::after(Duration::from_millis(1_000)).await;

    let mut socket = TcpSocket::new(stack, rx_buffer, tx_buffer);

    socket.set_timeout(Some(Duration::from_secs(100)));

    let remote_endpoint = (core::net::Ipv4Addr::new(192, 168, 1, 1), 1883);
    info!("connecting...");
    loop {
        let r = socket.connect(remote_endpoint).await;
        if let Err(e) = r {
            error!("connect error: {:?}", e);
        
        } else {
            break;
        }
    }

    info!("connected!");
    Timer::after(Duration::from_secs(1)).await;
    // break;

    mqtt::mqtt_task(socket).await;

    loop {
        info!("Got waiting");
        Timer::after(Duration::from_secs(1)).await;
        
    }
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    info!("start connection task");
    info!("Device capabilities: {:?}", controller.capabilities());
    loop {
        match esp_radio::wifi::sta_state() {
            WifiStaState::Connected => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
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
            let scan_config = ScanConfig::default().with_max(10);
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
