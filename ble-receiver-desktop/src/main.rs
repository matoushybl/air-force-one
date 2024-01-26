use afo_shared::AirQualityAdvertisement;
use btleplug::api::bleuuid::uuid_from_u16;
use postcard::from_bytes;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::time;

use btleplug::api::{Central, Manager as _, Peripheral, ScanFilter};
use btleplug::platform::Manager;

#[derive(Debug, serde::Deserialize)]
struct Measurement {
    data: [AirQualityAdvertisement; 2],
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    pretty_env_logger::init();

    let mut mqttoptions = MqttOptions::new("rumqtt-async", "10.15.0.4", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    tokio::task::spawn(async move {
        while let Ok(notification) = eventloop.poll().await {
            log::trace!("Received = {:?}", notification);
        }
    });

    loop {
        println!("tick");
        let mut socket = TcpStream::connect("10.42.0.61:1234").await?;
        let mut s = String::new();
        let mut buf = [0; 100];
        let n = socket.read(&mut buf).await?;
        let manuf = from_bytes::<Measurement>(&buf[..n]);
        match manuf {
            Ok(ref data) => {
                println!("Received data: {:?}", manuf);
                for datapoint in data.data {
                    client
                        .publish(
                            format!("afo-{}", datapoint.sensor_id),
                            QoS::AtMostOnce,
                            false,
                            format!(
                                r#"{{"co2": "{}", "temperature": "{}", "humidity": "{}"}}"#,
                                datapoint.co2_concentration,
                                datapoint.temperature as f32 * 0.1,
                                datapoint.humidity
                            ),
                        )
                        .await?;
                }
            }
            Err(_) => println!("Faild to parse manufacturer specific data."),
        }
        println!("received: {:?}", &buf);

        tokio::time::sleep(Duration::from_secs(10)).await;
    }

    // let manager = Manager::new().await?;
    // let adapter_list = manager.adapters().await?;
    // if adapter_list.is_empty() {
    //     return Err(eyre::eyre!("No bluetooth adapters found."));
    // }
    //
    // for adapter in adapter_list.iter() {
    //     loop {
    //         log::trace!("Starting scan on {}...", adapter.adapter_info().await?);
    //         let _: Result<_, _> = adapter.stop_scan().await;
    //         adapter
    //             .start_scan(ScanFilter {
    //                 services: vec![uuid_from_u16(0x181a)],
    //             })
    //             .await
    //             .expect("Can't scan BLE adapter for connected devices...");
    //         time::sleep(Duration::from_secs(1)).await;
    //         let peripherals = adapter.peripherals().await?;
    //         if peripherals.is_empty() {
    //             log::warn!("->>> BLE peripheral devices were not found, sorry. Exiting...");
    //         } else {
    //             for peripheral in peripherals.iter() {
    //                 let properties = peripheral.properties().await?;
    //                 let manuf = match properties.unwrap().manufacturer_data.get(&0xffff) {
    //                     Some(manuf) => manuf,
    //                     None => continue,
    //                 }
    //                 .clone();
    //                 let manuf = from_bytes::<AirQualityAdvertisement>(&manuf);
    //                 match manuf {
    //                     Ok(data) => {
    //                         log::info!("Received data: {:?}", manuf);
    //                         client.publish(format!("afo-{}", data.sensor_id), QoS::AtMostOnce, false, format!(r#"{{"co2": "{}", "temperature": "{}", "humidity": "{}"}}"#, data.co2_concentration, data.temperature as f32 * 0.1, data.humidity)).await?;
    //                     }
    //                     Err(_) => log::error!("Faild to parse manufacturer specific data."),
    //                 }
    //             }
    //         }
    //     }
    // }
    Ok(())
}
