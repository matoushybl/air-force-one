// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.

use btleplug::api::bleuuid::uuid_from_u16;
use postcard::from_bytes;
use std::error::Error;
use std::time::Duration;
use tokio::time;

use btleplug::api::{Central, Manager as _, Peripheral, ScanFilter};
use btleplug::platform::Manager;

use shared::{AirQuality, AirQualityAdvertisement};
use telegraf::{Client, Metric};

#[derive(Metric)]
struct CarbonDioxide {
    field1: f32,
    #[telegraf(tag)]
    tag1: String,
}

#[derive(Metric)]
struct AirQualityMetric {
    co2: f32,
    temperature: f32,
    humidity: f32,
    pub mass_pm1_0: f32,
    /// Mass Concentration PM2.5 [μg/m³]
    pub mass_pm2_5: f32,
    /// Mass Concentration PM4.0 [μg/m³]
    pub mass_pm4_0: f32,
    /// Mass Concentration PM10 [μg/m³]
    pub mass_pm10: f32,
    /// Number Concentration PM0.5 [#/cm³]
    pub number_pm0_5: f32,
    /// Number Concentration PM1.0 [#/cm³]
    pub number_pm1_0: f32,
    /// Number Concentration PM2.5 [#/cm³]
    pub number_pm2_5: f32,
    /// Number Concentration PM4.0 [#/cm³]
    pub number_pm4_0: f32,
    /// Number Concentration PM10 [#/cm³]
    pub number_pm10: f32,
    /// Typical Particle Size [μm]
    pub typical_particulate_matter_size: f32,
    pub voc_index: i32,
    #[telegraf(tag)]
    tag: String,
}

impl From<AirQuality> for AirQualityMetric {
    fn from(raw: AirQuality) -> Self {
        Self {
            tag: "default".to_string(),
            co2: raw.co2_concentration,
            temperature: raw.temperature,
            humidity: raw.humidity,
            mass_pm1_0: raw.mass_pm1_0,
            mass_pm2_5: raw.mass_pm2_5,
            mass_pm4_0: raw.mass_pm4_0,
            mass_pm10: raw.mass_pm10,
            number_pm0_5: raw.number_pm0_5,
            number_pm1_0: raw.number_pm1_0,
            number_pm2_5: raw.number_pm2_5,
            number_pm4_0: raw.number_pm4_0,
            number_pm10: raw.number_pm10,
            typical_particulate_matter_size: raw.typical_particulate_matter_size,
            voc_index: raw.voc_index as i32,
        }
    }
}

impl From<AirQualityAdvertisement> for AirQualityMetric {
    fn from(raw: AirQualityAdvertisement) -> Self {
        Self {
            co2: raw.co2_concentration as f32,
            temperature: raw.temperature as f32 * 0.1,
            humidity: raw.humidity as f32,
            mass_pm1_0: raw.mass_pm1_0 as f32,
            mass_pm2_5: raw.mass_pm2_5 as f32,
            mass_pm4_0: raw.mass_pm4_0 as f32,
            mass_pm10: raw.mass_pm10 as f32,
            number_pm0_5: 0.0,
            number_pm1_0: 0.0,
            number_pm2_5: 0.0,
            number_pm4_0: 0.0,
            number_pm10: 0.0,
            typical_particulate_matter_size: 0.0,
            voc_index: raw.voc_index as _,
            tag: "default".to_string(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let mut client = Client::new(&format!("tcp://localhost:8094")).unwrap();

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
                    let manuf = match properties.unwrap().manufacturer_data.get(&0xffff) {
                        Some(manuf) => manuf,
                        None => continue,
                    }
                    .clone();
                    let manuf = from_bytes::<shared::AirQualityAdvertisement>(&manuf);
                    match manuf {
                        Ok(data) => {
                            let point = AirQualityMetric::from(data);
                            client.write(&point).unwrap();
                            println!("{:?}", data);
                        }
                        Err(_) => println!("Faild to parse manufacturer specific bs"),
                    }
                    // println!(
                    //     "Peripheral {:?} {:x?} is connected: {:?}",
                    //     0, local_name, is_connected
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
