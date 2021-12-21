use serial2::SerialPort;
use shared::AirQuality;
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
    #[telegraf(tag)]
    tag1: String,
}

fn main() {
    println!("Hello, world!");

    let mut client = Client::new("tcp://10.15.0.15:8094").unwrap();
    let point = CarbonDioxide {
        field1: 1800.234,
        tag1: "afo1".to_string(),
    };
    client.write(&point).unwrap();
    let port = SerialPort::open("/dev/cu.usbmodemAFO1", 115200).unwrap();
    let mut buffer = [0; 256];
    let mut rolling_buffer = Vec::<u8>::new();
    loop {
        if let Ok(read) = port.read(&mut buffer) {
            println!("read {:?}", read);
            rolling_buffer.extend_from_slice(&buffer[..read]);
            if rolling_buffer.contains(&0) {
                if let Ok(decoded) =
                    postcard::from_bytes_cobs::<AirQuality>(&mut rolling_buffer[..])
                {
                    println!("{:?}", decoded);
                    let point = AirQualityMetric {
                        tag1: "afo1".to_string(),
                        co2: decoded.co2_concentration,
                        temperature: decoded.temperature,
                        humidity: decoded.humidity,
                        mass_pm1_0: decoded.mass_pm1_0,
                        mass_pm2_5: decoded.mass_pm2_5,
                        mass_pm4_0: decoded.mass_pm4_0,
                        mass_pm10: decoded.mass_pm10,
                        number_pm0_5: decoded.number_pm0_5,
                        number_pm1_0: decoded.number_pm1_0,
                        number_pm2_5: decoded.number_pm2_5,
                        number_pm4_0: decoded.number_pm4_0,
                        number_pm10: decoded.number_pm10,
                        typical_particulate_matter_size: decoded.typical_particulate_matter_size,
                    };
                    client.write(&point).unwrap();
                }
                rolling_buffer.clear();
            }
        }
    }
}

// struct Reader<R: std::io::Read> {
//     buffer: Vec<u8>,
//     end: usize,
// }

// impl<R: std::io::Read> Reader<R> {
//     pub fn new(buffer_size: usize) -> Self {
//         Self {
//             buffer: vec![0u8; buffer_size],
//             end: 0,
//         }
//     }

//     pub fn contains(&self, data: &u8) -> bool {
//         self.buffer[..end].contains(data)
//     }
// }

// impl<R: std::io::Read> std::io::Read for Reader<R> {
//     fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
//         todo!()
//     }
// }
