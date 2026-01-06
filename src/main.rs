#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use defmt::info;
use defmt::error;
use embassy_executor::Spawner;
use embassy_net::DhcpConfig;
use embassy_net::{
    dns::DnsSocket,
    tcp::TcpSocket, 

    tcp::client::{TcpClient, TcpClientState},
    
    Stack,
    StackResources,
    Runner,
};
use esp_radio::wifi::{
    ClientConfig,
    ModeConfig,
    ScanConfig,
    WifiController,
    WifiDevice,
    WifiEvent,
    WifiStaState,
};
use embassy_time::{Duration, Timer};
use esp_hal::{
    
    clock::CpuClock,
    ram,
    rng::Rng,
    timer::timg::TimerGroup
};
use {esp_backtrace as _, esp_println as _};
use reqwless::client::{HttpClient, TlsConfig};
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

    let cr = mk_static!(esp_radio::Controller<'static> , esp_radio::init().unwrap());
    let (mut controller, interfaces) =
        esp_radio::wifi::new(&*cr, peripherals.WIFI, Default::default()).unwrap();

    controller.set_power_saving(esp_radio::wifi::PowerSaveMode::Minimum).unwrap();
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

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];

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

    loop {
        Timer::after(Duration::from_millis(1_000)).await;

        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        socket.set_timeout(Some(Duration::from_secs(10)));

        let remote_endpoint = (core::net::Ipv4Addr::new(142, 250, 185, 115), 80);
        info!("connecting...");
        let r = socket.connect(remote_endpoint).await;
        if let Err(e) = r {
            error!("connect error: {:?}", e);
            continue;
        }
        // loop {
        
            info!("connected!");
            // Timer::after(Duration::from_secs(10)).await

        // }
        // break;
        access_website(stack, tls_seed).await;
        Timer::after(Duration::from_secs(1)).await;

    //     let mut buf = [0; 1024];
    //     loop {
    //         // use embedded_io_async::Write;
    //         let r = socket
    //             .write(b"GET / HTTP/1.0\r\nHost: aeknyy-de.duckdns.org\r\n\r\n")
    //             .await;
    //         if let Err(e) = r {
    //             error!("write error: {:?}", e);
    //             break;
    //         }
    //         let n = match socket.read(&mut buf).await {
    //             Ok(0) => {
    //                 info!("read EOF");
    //                 break;
    //             }
    //             Ok(n) => n,
    //             Err(e) => {
    //                 error!("read error: {:?}", e);
    //                 break;
    //             }
    //         };
    //         info!("{}", core::str::from_utf8(&buf[..n]).unwrap());
    //     }
    //     Timer::after(Duration::from_millis(3000)).await;
    // }


    // // TODO: Spawn some tasks
    // let _ = spawner;

    // loop {
    //     info!("Hello world!");
    //     Timer::after(Duration::from_secs(1)).await;
    // }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0/examples/src/bin
}}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    info!("start connection task");
    info!("Device capabilities: {:?}", controller.capabilities());
    loop {
        match esp_radio::wifi::sta_state() {
            WifiStaState::Connected => {
                // wait until we're no longer connected
                controller
                    .wait_for_event(WifiEvent::StaDisconnected)
                    .await;
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

async fn access_website(stack: Stack<'_>, tls_seed: u64) {
    let mut rx_buffer = [0; 16386];
    let mut tx_buffer = [0; 4096];
    let dns = DnsSocket::new(stack);
    let tcp_state = TcpClientState::<1, 4096, 16386>::new();
    let tcp = TcpClient::new(stack, &tcp_state);

    let tls = TlsConfig::new(
        tls_seed,
        &mut rx_buffer,
        &mut tx_buffer,
        reqwless::client::TlsVerify::None,
    );

    let mut client = HttpClient::new_with_tls(&tcp, &dns, tls);
    let mut buffer = [0u8; 16386];
    // let mut http_req = client
    //     .request(
    //         reqwless::request::Method::GET,
    //         "https://aeknyy-de.duckdns.org/",
    //     )
    //     .await
    //     .unwrap();
    // let response = http_req.send(&mut buffer).await;
    loop {
        let mut http_req = match client
        .request(
            reqwless::request::Method::GET,
            "https://aeknyy-de.duckdns.org/",
        )
        .await {
            Ok(a) => a,
            Err(_) => {
                Timer::after(Duration::from_secs(1)).await;
                info!("Continuing request");
                break;
            },
        };
        

        loop {

            Timer::after(Duration::from_secs(1)).await;

            let respose = match http_req.send(&mut buffer).await {
                Ok(a) => a,
                Err(_) => {
                    // Timer::after(Duration::from_secs(1)).await;
                    info!("Continuing send");
                    break;
                },
            };
            
            info!("{:?}", respose.status.0);
            // Timer::after(Duration::from_secs(1)).await;
            
            // info!("Reading");
            // let answer = respose.body().read_to_end().await;
            // info!("Have been read");
            // let answer = match answer {
            //     Ok(a) => info!("Done"),
            //     Err(_) => {
            //         error!("Error reading!");
            //     },
            // };
            // info!("{}", core::str::from_utf8(&buffer[..(respose.content_length)]).unwrap());
            // let s_c = &respose.status.0;
            // info!("Got response {:?}", s_c);

            // loop {
                // match respose.body().read_to_end().await {
                //     Ok(a) => info!("{:?}", a),
                //     Err(_) => {
                //         info!("Continuing read");
                //         continue
                //     },
                // }
            // };
            

            
            break;

        }
        Timer::after(Duration::from_secs(1)).await;
        

        
    }
}

    

    
    // loop {
    //     match response.await {
    //         // Ok(0) => break,  // Connection closed
    //         Ok(r) => {
                
    //             // Optionally parse/print response
    //             info!("Read {:?}", r.);
    //         }
    //         Err(embedded_tls::TlsError::WouldBlock) => {
    //             // Normal in non-blocking mode — just try again
    //             continue;
    //         }
    //         Err(e) => return Err(e),
    //     }
    // }
    // let res = response.body().read_to_end().await.unwrap();

    // let content = core::str::from_utf8(res).unwrap();
    // info!("{}", content);
// }

