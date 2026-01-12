// use core::ptr::read_volatile;

use alloc::boxed::Box;
// use alloc::boxed::Box;
use defmt::{error, info};
use embassy_executor::task;
use embassy_net::tcp::TcpSocket;
use embassy_time::{Duration, Timer};
use rust_mqtt::{
    buffer:: BumpBuffer, 
    client::{Client, options::{ConnectOptions, WillOptions}}, 
    config::{KeepAlive, SessionExpiryInterval}, 
    types::{MqttBinary, MqttString, QoS}};
use smoltcp::socket;

use {esp_backtrace as _, esp_println as _};

// use embedded_nal_async::TcpConnect;
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}
// #[task]
pub async fn mqtt_task(socket: TcpSocket<'static>) {
    // let mut buf = [0u8; 1024];

    // let s = Box::new(socket);
    // let stb = mk_static!([u8; 1024], buf);
    // let socket_ = mk_static!(TcpSocket, socket);
    let stb2 = mk_static!([u8; 1024], [0; 1024]);
    let y = BumpBuffer::new( stb2);
    let b = mk_static!(BumpBuffer, y);

    // let mut buf = [0u8; 1024];
    // let x = Box::new(BumpBuffer::new(stb));

    // let b_buff: &'static mut BumpBuffer = Box::leak(x);

    let mut client = Client::<
        'static,
        TcpSocket<'static>,
        BumpBuffer<'static>, 
        1, 
        1, 
        1
        >
        ::new(b);

    let o = ConnectOptions {
                session_expiry_interval: SessionExpiryInterval::Seconds(5),
                clean_start: false,
                keep_alive: KeepAlive::Seconds(3),
                will: Some(WillOptions {
                    will_qos: QoS::ExactlyOnce,
                    will_retain: true,
                    will_topic: MqttString::try_from("dead").unwrap(),
                    will_payload: MqttBinary::try_from("joe mama").unwrap(),
                    will_delay_interval: 10,
                    is_payload_utf8: true,
                    message_expiry_interval: Some(20),
                    content_type: Some(MqttString::try_from("txt").unwrap()),
                    response_topic: None,
                    correlation_data: None,
                }),
                user_name: Some(MqttString::try_from("test").unwrap()),
                password: Some(MqttBinary::try_from("testPass").unwrap()),
            };

    
    // loop {
    Timer::after(Duration::from_secs(1)).await;

    let c_info = client.connect(
        socket, 
        &o, 
        Some(MqttString::try_from("rust-mqtt-demo-client").unwrap()
        )
    )
    .await;
    match c_info {
        Ok(_) => info!("Mqtt connected"),
        Err(e) => {
            error!("Mqtt not connected");
        }
    };
    // }
    
}
