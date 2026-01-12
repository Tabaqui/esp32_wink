use defmt::{error, info};
use embassy_executor::task;
use embassy_net::{Stack, tcp::TcpSocket};
use embassy_time::{Duration, Timer};
use rust_mqtt::{
    buffer::BumpBuffer,
    client::{
        Client,
        options::{ConnectOptions, WillOptions},
    },
    config::{KeepAlive, SessionExpiryInterval},
    types::{MqttBinary, MqttString, QoS},
};

use {esp_backtrace as _, esp_println as _};

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write($val);
        x
    }};
}

#[task]
pub async fn mqtt_task(
    stack: Stack<'static>,
    mqtt_buffer: &'static mut [u8]
) {

    loop {
        if stack.is_config_up() {
            break;
        }
        info!("waiting for stack to get ready");
        Timer::after(Duration::from_secs(10)).await;
    }

    let mut rx_buffer = [0u8; 4096];
    let mut tx_buffer = [0u8; 4096];

    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

    socket.set_timeout(Some(Duration::from_secs(100)));

    let remote_endpoint = (core::net::Ipv4Addr::new(192, 168, 1, 1), 1883);
    info!("connecting...");
    loop {
        let r = socket.connect(remote_endpoint).await;
        if let Err(e) = r {
            error!("connect error: {:?}", e);
            Timer::after(Duration::from_secs(5)).await;
        } else {
            break;
        }
    }

    info!("TCP connected!");

    let mqtt_bump = BumpBuffer::new(mqtt_buffer);
    let mqtt_static = mk_static!(BumpBuffer, mqtt_bump);

    let mut client = Client::<'static, TcpSocket<'static>, BumpBuffer<'static>, 1, 1, 1>::new(mqtt_static);

    let o = ConnectOptions {
        session_expiry_interval: SessionExpiryInterval::Seconds(600),
        clean_start: false,
        keep_alive: KeepAlive::Seconds(120),
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
        user_name: None,
        password: None,
    };

    Timer::after(Duration::from_secs(1)).await;

    let c_info = client
        .connect(
            socket,
            &o,
            Some(MqttString::try_from("rust-mqtt-demo-client").unwrap()),
        )
        .await;

        unsafe {
            client.buffer().reset();
        }

    match c_info {
        Ok(_) => info!("Mqtt connected"),
        Err(_) => {
            error!("Mqtt not connected");
        }
    };
}
