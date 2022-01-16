use std::time::Duration;

use shared::AirQuality;
use telegraf::{Client, Metric};
use tokio::io::AsyncReadExt;
use tokio_serial::{SerialPortBuilderExt, SerialStream};

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

use clap::Parser;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Config {
    port: String,
    tag: String,
    telegraf_address: Option<String>,
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");
    let config = Config::parse();

    let mut client = Client::new(&format!(
        "tcp://{}",
        config
            .telegraf_address
            .unwrap_or("localhost:8094".to_string()),
    ))
    .unwrap();

    loop {
        match tokio_serial::new(config.port.as_str(), 115200).open_native_async() {
            Ok(port) => handle_serial_port(port, &mut client, config.tag.as_str()).await,
            Err(_) => println!("Failed to open serial port."),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn handle_serial_port(mut port: SerialStream, client: &mut Client, tag: &str) {
    // TODO rewrite using tokio codec
    let mut buffer = [0; 256];
    let mut rolling_buffer = Vec::<u8>::new();
    loop {
        let read_result =
            tokio::time::timeout(Duration::from_secs(10), port.read(&mut buffer)).await;
        if let Ok(Ok(read)) = read_result {
            if read == 0 {
                return;
            }
            rolling_buffer.extend_from_slice(&buffer[..read]);
            if rolling_buffer.contains(&0) {
                if let Ok(decoded) =
                    postcard::from_bytes_cobs::<AirQuality>(&mut rolling_buffer[..])
                {
                    println!("{:?}", decoded);
                    let mut point = AirQualityMetric::from(decoded);
                    point.tag = tag.to_string();
                    client.write(&point).unwrap();
                }
                rolling_buffer.clear();
            }
        } else {
            return;
        }
    }
}
