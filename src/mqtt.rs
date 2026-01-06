use embassy_executor::task;
use embassy_net::tcp::TcpSocket;
// use rust_mqtt::{buffer::AllocBuffer, client::Client};
use {esp_backtrace as _, esp_println as _};

// use embedded_nal_async::TcpConnect;


#[task]
pub async fn mqtt_task() {
    // TcpSocket
    // let mut a_buf = AllocBuffer;
    // let client: Client<'_, TcpSocket<'_>, _, 1, 1, 1> = Client::new(&mut a_buf);
}
