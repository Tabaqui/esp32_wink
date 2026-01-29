use core::net::Ipv4Addr;

use core::result::Result::*;
use defmt::{error, info, warn};
use embassy_executor::task;
use embassy_net::{Stack, tcp::TcpSocket};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Receiver, Sender};
use embassy_time::{Duration, Timer};
use rust_mqtt::client::event::Publish;

use crate::led::Light;

use rust_mqtt::Bytes;
use rust_mqtt::{
    buffer::BumpBuffer,
    client::{
        Client, MqttError,
        event::{Event, Suback},
        options::{
            ConnectOptions, DisconnectOptions, PublicationOptions, RetainHandling,
            SubscriptionOptions, WillOptions,
        },
    },
    config::{KeepAlive, SessionExpiryInterval},
    types::{MqttBinary, MqttString, QoS, TopicName},
};

use {esp_backtrace as _, esp_println as _};

type MyMqttClient<'a> = Client<'a, TcpSocket<'a>, BumpBuffer<'a>, 1, 1, 1>;

const REMOTE_ENDPOINT: (Ipv4Addr, u16) = (Ipv4Addr::new(192, 168, 1, 1), 1883);

#[task]
pub async fn mqtt_task(
    stack: Stack<'static>,
    l_sen: &'static Sender<'static, NoopRawMutex, Light, 3>,
) {
    wait_ip(stack).await;

    let o = ConnectOptions {
        session_expiry_interval: SessionExpiryInterval::Seconds(600),
        clean_start: false,
        keep_alive: KeepAlive::Seconds(600),
        will: Some(WillOptions {
            will_qos: QoS::ExactlyOnce,
            will_retain: true,
            will_topic: MqttString::try_from("el").unwrap(),
            will_payload: MqttBinary::try_from("joe mama").unwrap(),
            will_delay_interval: 10,
            is_payload_utf8: true,
            message_expiry_interval: Some(20),
            content_type: Some(MqttString::try_from("cbor/application").unwrap()),
            response_topic: None,
            correlation_data: None,
        }),
        user_name: None,
        password: None,
    };

    let topic = unsafe { TopicName::new_unchecked(MqttString::from_slice("el").unwrap()) };

    loop {
        let mut rx_buffer = [0u8; 4096];
        let mut tx_buffer = [0u8; 4096];

        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        socket.set_timeout(Some(Duration::from_secs(1000)));

        let mut mqtt_buffer = [0u8; 1024];
        let mut mqtt_bump = BumpBuffer::new(&mut mqtt_buffer);

        Timer::after(Duration::from_secs(1)).await;

        let mut client = Client::<'_, _, _, 1, 1, 1>::new(&mut mqtt_bump);

        Timer::after(Duration::from_secs(1)).await;

        connet_tcp_async(&mut socket).await;

        mqtt_connect_async(socket, &mut client, &o).await;

        subscribe_n_cofirm_async(topic.clone(), &mut client, l_sen).await;

        // publish_n_confirm_async(l_rec, topic.clone(), &mut client).await;

        // if (poll_async(&mut client, l_sen).await).is_err() {
        //     unsafe {
        //         client.buffer().reset();
        //     }
        //     client.abort().await;
        //     continue;
        // }

        unsafe {
            client.buffer().reset();
        }

        disconnect_gracefully_async(&mut client).await;
        // break;
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

async fn connet_tcp_async<'a>(socket: &mut TcpSocket<'a>) {
    info!("connecting...");
    loop {
        let r = socket.connect(REMOTE_ENDPOINT).await;
        if let Err(e) = r {
            error!("connect error: {:?}", e);
            Timer::after(Duration::from_secs(5)).await;
        } else {
            info!("TCP connected!");
            return;
        }
    }
}

async fn mqtt_connect_async<'a>(
    socket: TcpSocket<'a>,
    client: &mut MyMqttClient<'a>,
    o: &ConnectOptions<'a>,
) {
    let c_info = client
        .connect(
            socket,
            o,
            Some(MqttString::try_from("rust-mqtt-demo-client").unwrap()),
        )
        .await;

    match c_info {
        Ok(_) => info!("Mqtt connected"),
        Err(_) => {
            error!("Mqtt not connected");
        }
    };
}

async fn subscribe_n_cofirm_async<'a>(topic: TopicName<'a>, client: &mut MyMqttClient<'a>, l_sen: &'static Sender<'static, NoopRawMutex, Light, 3>) {
    
    let sub_options = SubscriptionOptions {
        retain_handling: RetainHandling::AlwaysSend,
        retain_as_published: false,
        no_local: false,
        qos: QoS::AtMostOnce ,
    };

    let sub_id = match client.subscribe(topic.into(), sub_options).await {
        Ok(id) => {
            info!("Sent Subscribe");
            id
        }
        Err(e) => {
            error!("Failed to subscribe: {:?}", e);
            return;
        }
    };

    loop {
        info!("poll");
        match client.poll().await {
            Ok(Event::Suback(Suback {
                packet_identifier,
                reason_code,
            })) => {
                info!(
                    "Subscribed with reason code {:?} with id: {:?}",
                    reason_code, packet_identifier
                );
                // break;
            }
            Ok(Event::Publish(Publish {
                identified_qos,
                dup,
                retain,
                topic,
                message,
            })) => { 
                warn!("Event after subscribing: {:?}", identified_qos);
                let msg = minicbor::decode::<Light>(&message);
                info!("{:?}", message);
                if let Ok(l) = msg {    
                    // error!("error decode {:?}", m);
                    l_sen.send(l).await;
                    Timer::after(Duration::from_millis(200)).await;

                }                 
            }
            Ok(e) => {
                warn!("Expected Suback but received event {:?}", e);
            }
            Err(e) => {
                error!("Failed to receive Suback {:?}", e);
            }
        }
    }
}

async fn publish_n_confirm_async<'a>(
    l_rec: &Receiver<'a, NoopRawMutex, Light, 3>,
    topic: TopicName<'a>,
    client: &mut MyMqttClient<'a>,
) {
    let pub_options = PublicationOptions {
        retain: false,
        topic,
        qos: QoS::ExactlyOnce,
    };

    let mut buffer = [0u8; 32];
    let mut current_light_num = 0;

    loop {
        let message = l_rec.receive().await;
        info!("received {:?}", message);

        minicbor::encode(&message, buffer.as_mut()).unwrap();
        info!("Encoded! {}", Bytes::from(buffer.as_mut()));

        match client
            // .publish(&pub_options, Bytes::from(vzed.deref()))
            .publish(&pub_options, Bytes::from(buffer.as_mut()))
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

        loop {
            match client.poll().await {
                Ok(Event::PublishComplete(_)) => {
                    info!("Publish complete");
                    break;
                }
                Ok(e) => warn!("Received event {:?}", e),
                Err(e) => {
                    error!("Failed to poll!: {:?}", e);
                    Timer::after(Duration::from_secs(1)).await;
                    return;
                }
            }
        }
        current_light_num += 1;
        Timer::after(Duration::from_micros(100)).await;
        if current_light_num > 2 {
            break;
        }
    }
}

async fn poll_async<'a>(
    client: &mut MyMqttClient<'a>,
    l_sen: &'static Sender<'static, NoopRawMutex, Light, 3>,
) -> Result<(), MqttError<'a>> {
    match client.poll().await {
        Ok(e) => {
            info!("Received Event {:?}", e);
            if let Event::Publish(publish) = e {
                let res = minicbor::decode::<Light>(&publish.message);
                match res {
                    Ok(light) => l_sen.send(light).await,
                    Err(_) => todo!(),
                }
                todo!()
            }
        }
        Err(e) => {
            error!("Failed to poll!!: {:?}", e);
            if !e.is_recoverable() {
                error!("Err is not recoverable");
                Timer::after(Duration::from_secs(1)).await;
                return Err(e);
            }
        }
    }

    Timer::after(Duration::from_millis(100)).await;
    Ok(())
}

async fn disconnect_gracefully_async<'a>(client: &mut MyMqttClient<'a>) {
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
        }
    }
}
