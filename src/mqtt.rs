use embassy_executor::task;
use embassy_net::tcp::TcpSocket;

use esp_hal::rng::Rng;
use rust_mqtt::client::{client::MqttClient, client_config::{ClientConfig, MqttVersion}};
use smoltcp::socket;
use {esp_backtrace as _, esp_println as _};

// use embedded_nal_async::TcpConnect;


#[task]
pub async fn mqtt_task(socket: TcpSocket<'static>) {
    // TcpSocket
    // let mut a_buf = AllocBuffer;
    let mut s_buff = [0; 1024];
    let ss = s_buff.len().clone();
    let mut r_buff = [0; 1024];
    let rr = r_buff.len().clone();

    MqttClient::<_, _, _>::new(
        socket,
        &mut s_buff,
        ss,
        &mut r_buff, 
        rr, 
        ClientConfig::<5, Rng>::new(MqttVersion::MQTTv5, Rng::new())
    );
}
