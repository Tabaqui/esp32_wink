use defmt::{error, info};
use embassy_executor::task;
use embassy_futures::select::select;
use embassy_net::{Stack, tcp::TcpSocket};
use embassy_time::{Duration, Timer};
use embassy_futures::select::Either::*;
use rust_mqtt::{
    Bytes, buffer::BumpBuffer, client::{
        Client, event::{Event, Suback}, options::{ConnectOptions, DisconnectOptions, PublicationOptions, RetainHandling, SubscriptionOptions, WillOptions}
    }, config::{KeepAlive, SessionExpiryInterval}, types::{MqttBinary, MqttString, QoS, TopicName}
};
use smoltcp::socket;
use core::net::Ipv4Addr;

use {esp_backtrace as _, esp_println as _};

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write($val);
        x
    }};
}

const REMOTE_ENDPOINT: (Ipv4Addr, u16) = (Ipv4Addr::new(192, 168, 1, 1), 1883);


#[task]
pub async fn mqtt_task(stack: Stack<'static>, mqtt_buffer: &'static mut [u8]) {
    wait_ip(stack).await;
    let socket = get_socket(stack);



    let socket = connet_tcp(socket).await;


    let mqtt_bump = BumpBuffer::new(mqtt_buffer);
    let mqtt_static = mk_static!(BumpBuffer, mqtt_bump);

    let mut client =
        Client::<'static, TcpSocket<'static>, BumpBuffer<'static>, 1, 1, 1>::new(mqtt_static);

    let o = ConnectOptions {
        session_expiry_interval: SessionExpiryInterval::Seconds(600),
        clean_start: false,
        keep_alive: KeepAlive::Seconds(120),
        will: Some(WillOptions {
            will_qos: QoS::ExactlyOnce,
            will_retain: true,
            will_topic: MqttString::try_from("el").unwrap(),
            will_payload: MqttBinary::try_from("joe mama").unwrap(),
            will_delay_interval: 10,
            is_payload_utf8: true,
            message_expiry_interval: Some(20),
            content_type: Some(MqttString::try_from("text/plain").unwrap()),
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

    // let sub_options = SubscriptionOptions {
    //     retain_handling: RetainHandling::SendIfNotSubscribedBefore,
    //     retain_as_published: true,
    //     no_local: false,
    //     qos: QoS::ExactlyOnce,
    //     // subscription_identifier,
    // };

    let topic =
        unsafe { 
            TopicName::new_unchecked(MqttString::from_slice("el").unwrap()) 
        };

    // match client.subscribe(topic.clone().into(), sub_options).await {
    //     Ok(_) => info!("Sent Subscribe"),
    //     Err(e) => {
    //         error!("Failed to subscribe: {:?}", e);
    //         // return;
    //     }
    // };


    // match client.poll().await {
    //     Ok(Event::Suback(Suback {
    //         packet_identifier: _,
    //         reason_code,
    //     })) => info!("Subscribed with reason code {:?}", reason_code),
    //     Ok(e) => {
    //         error!("Expected Suback but received event {:?}", e);
    //         // return;
    //     }
    //     Err(e) => {
    //         error!("Failed to receive Suback {:?}", e);
    //         // return;
    //     }
    // }

    let pub_options = PublicationOptions {
        retain: false,
        // message_expiry_interval: None,
        topic: topic.clone(),
        qos: QoS::ExactlyOnce,
    };

    let publish_packet_id = match client
        .publish(&pub_options, Bytes::from("anything".as_bytes()))
        .await
    {
        Ok(i) => {
            info!("Published message with packet identifier {}", i);
            i
        }
        Err(e) => {
            error!("Failed to send Publish {:?}", e);
            return;
        }
    };

    // let pub_options = PublicationOptions {
    //     retain: false,
    //     // message_expiry_interval: None,
    //     topic: topic.clone(),
    //     qos: QoS::ExactlyOnce,
    // };
    // client
    //     .republish(
    //         publish_packet_id,
    //         &pub_options,
    //         Bytes::from("anything".as_bytes()),
    //     )
    //     .await
        // .unwrap();

    loop {
        match client.poll().await {
            Ok(Event::PublishComplete(_)) => {
                info!("Publish complete");
                break;
            }
            Ok(e) => info!("Received event {:?}", e),
            Err(e) => {
                error!("Failed to poll: {:?}", e);
                return;
            }
        }
    }

    // let t1 = Timer::after(Duration::from_secs(4));
    // let t2 = client.poll_header();

    // let sel = select(t1 , t2).await;

    // let (timer, poll_header) = sel;

    // match sel {
    //     First(_) => info!("has waited 4 secs"),
    //     Second(r_h) => {
    //         let h = match r_h {
    //             Ok(fh) => fh,
    //             Err(e) => {
    //                 error!("Failed to poll header: {:?}", e);
    //                 return;
    //             },
    //         };
    //     },
    // }

    // let mut pings = 3;

    // while pings > 0 {
    //     select {
    //         _ = sleep(Duration::from_secs(4)) => {
    //             match client.ping().await {
    //                 Ok(_) => {
    //                     pings -= 1;
    //                     info!("Pinged server");
    //                 },
    //                 Err(e) => {
    //                     error!("Failed to ping: {:?}", e);
    //                     return;
    //                 }
    //             }
    //         },
    //         header = client.poll_header() => {
    //             let h = match header {
    //                 Ok(h) => h,
    //                 Err(e) => {
    //                     error!("Failed to poll header: {:?}", e);
    //                     return;
    //                 }
    //             };
    //             info!("Received header {:?}", h.packet_type());
    //             match client.poll_body(h).await {
    //                 Ok(e) => info!("Received Event {:?}", e),
    //                 Err(e) => {
    //                     error!("Failed to poll body: {:?}", e);
    //                     return;
    //                 }
    //             }
    //         }
    //     };
    // }

    match client.poll().await {
        Ok(e) => info!("Received Event {:?}", e),
        Err(e) => {
            error!("Failed to poll: {:?}", e);
            return;
        }
    }

    // match client.unsubscribe(topic.clone().into()).await {
    //     Ok(_) => info!("Sent Unsubscribe"),
    //     Err(e) => {
    //         error!("Failed to unsubscribe: {:?}", e);
    //         return;
    //     }
    // };

    // match client.poll().await {
    //     Ok(Event::Unsuback(Suback {
    //         packet_identifier: _,
    //         reason_code,
    //     })) => info!("Unsubscribed with reason code {:?}", reason_code),
    //     Ok(e) => {
    //         info!("Expected Unsuback but received event {:?}", e);
    //         return;
    //     }
    //     Err(e) => {
    //         error!("Failed to receive Unsuback {:?}", e);
    //         return;
    //     }
    // }

    // let pub_options = PublicationOptions {
    //     retain: false,
    //     // message_expiry_interval: None,
    //     topic: topic.clone(),
    //     qos: QoS::AtMostOnce,
    // };

    // match client
    //     .publish(&pub_options, Bytes::from("something".as_bytes()))
    //     .await
    // {
    //     Ok(_) => info!("Published to topic alias 1 aka \"rust-mqtt/is/great\""),
    //     Err(e) => {
    //         error!("Failed to publish to topic alias {:?}", e);
    //         return;
    //     }
    // };
    

    match client
        .disconnect(&DisconnectOptions {
            publish_will: false,
            session_expiry_interval: None,
        })
        .await
    {
        Ok(_) => info!("Disconnected from server"),
        Err(e) => {
            error!("Failed to disconnect from server: {:?}", e);
            // return;
        }
    }
}

async fn wait_ip(stack: Stack<'static>) {
    loop {
        if stack.is_config_up() {
            break;
        }
        info!("waiting for the stack to get ready");
        Timer::after(Duration::from_secs(10)).await;
    }
}

fn get_socket<'a>(stack: Stack<'static>) -> TcpSocket<'a> {

    let rx_buffer = mk_static!([u8; 4096], [0u8; 4096]);
    let tx_buffer = mk_static!([u8; 4096], [0u8; 4096]);

    let mut socket = TcpSocket::new(stack, rx_buffer, tx_buffer);

    socket.set_timeout(Some(Duration::from_secs(100)));

    socket
}

async fn connet_tcp(mut socket: TcpSocket<'static>) -> TcpSocket<'static> {
    info!("connecting...");
    loop {
        let r = socket.connect(REMOTE_ENDPOINT).await;
        if let Err(e) = r {
            error!("connect error: {:?}", e);
            Timer::after(Duration::from_secs(5)).await;
        } else {
            info!("TCP connected!");
            return socket;
        }
    }
}
