// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.

use btleplug::api::bleuuid::uuid_from_u16;
use postcard::from_bytes;
use std::error::Error;
use std::time::Duration;
use tokio::time;

use btleplug::api::{Central, Manager as _, Peripheral, ScanFilter};
use btleplug::platform::Manager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let manager = Manager::new().await?;
    let adapter_list = manager.adapters().await?;
    if adapter_list.is_empty() {
        eprintln!("No Bluetooth adapters found");
    }

    for adapter in adapter_list.iter() {
        loop {
            println!("Starting scan on {}...", adapter.adapter_info().await?);
            adapter
                .start_scan(ScanFilter {
                    services: vec![uuid_from_u16(0x1809)],
                })
                .await
                .expect("Can't scan BLE adapter for connected devices...");
            time::sleep(Duration::from_secs(1)).await;
            let peripherals = adapter.peripherals().await?;
            if peripherals.is_empty() {
                eprintln!("->>> BLE peripheral devices were not found, sorry. Exiting...");
            } else {
                // All peripheral devices in range
                for peripheral in peripherals.iter() {
                    let properties = peripheral.properties().await?;
                    let is_connected = peripheral.is_connected().await?;
                    let local_name = properties
                        .clone()
                        .unwrap()
                        .local_name
                        .unwrap_or(String::from("(peripheral name unknown)"));
                    let manuf = properties
                        .unwrap()
                        .manufacturer_data
                        .get(&0xffff)
                        .unwrap()
                        .clone();
                    let manuf = from_bytes::<shared::AirQualityAdvertisement>(&manuf);
                    match manuf {
                        Ok(data) => println!("{:?}", data),
                        Err(_) => println!("Faild to parse manufacturer specific bs"),
                    }
                    // println!(
                    //     "Peripheral {:?} {:x?} is connected: {:?}",
                    //     manuf, local_name, is_connected
                    // );
                    // if !is_connected {
                    //     println!("Connecting to peripheral {:?}...", &local_name);
                    //     if let Err(err) = peripheral.connect().await {
                    //         eprintln!("Error connecting to peripheral, skipping: {}", err);
                    //         continue;
                    //     }
                    // }
                    // let is_connected = peripheral.is_connected().await?;
                    // println!(
                    //     "Now connected ({:?}) to peripheral {:?}...",
                    //     is_connected, &local_name
                    // );
                    // peripheral.discover_services().await?;
                    // println!("Discover peripheral {:?} services...", &local_name);
                    // for service in peripheral.services() {
                    //     println!(
                    //         "Service UUID {}, primary: {}",
                    //         service.uuid, service.primary
                    //     );
                    //     for characteristic in service.characteristics {
                    //         println!("  {:?}", characteristic);
                    //     }
                    // }
                    // if is_connected {
                    //     println!("Disconnecting from peripheral {:?}...", &local_name);
                    //     peripheral
                    //         .disconnect()
                    //         .await
                    //         .expect("Error disconnecting from BLE peripheral");
                    // }
                }
            }
        }
    }
    Ok(())
}
