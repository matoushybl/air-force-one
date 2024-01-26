#![cfg_attr(not(test), no_std)]

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default)]
pub struct Co2(pub f32);

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default)]
pub struct Temperature(pub f32);

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default)]
pub struct Humidity(pub f32);

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Copy, Clone, Debug, Default)]
pub struct AirQuality {
    pub co2: Co2,
    pub temperature: Temperature,
    pub humidity: Humidity,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(
    Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq, Default, Clone, Copy,
)]
pub struct AirQualityAdvertisement {
    pub sensor_id: u8,
    pub co2_concentration: u16,
    pub temperature: i16, // scaled by 0.1
    pub humidity: u8,
}

impl From<(u8, AirQuality)> for AirQualityAdvertisement {
    fn from((id, raw): (u8, AirQuality)) -> Self {
        AirQualityAdvertisement {
            co2_concentration: raw.co2.0 as u16,
            temperature: (raw.temperature.0 / 0.1) as i16,
            humidity: raw.humidity.0 as u8,
            sensor_id: id,
        }
    }
}

/// Fills adv data with data type, length and data, return size of the filled data
pub fn fill_adv_data(adv_data: &mut [u8], data_type: u8, data: &[u8]) -> usize {
    let offset = 1 + data.len();
    adv_data[0] = offset as u8;
    adv_data[1] = data_type;
    adv_data[2..][..data.len()].copy_from_slice(data);
    offset + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use postcard::to_slice;

    #[test]
    fn adv() {
        let data = AirQualityAdvertisement {
            co2_concentration: 400,
            temperature: 225,
            humidity: 40,
            sensor_id: 0,
        };
        let mut buffer = [0u8; 100];
        let output = to_slice(&data, &mut buffer).unwrap();

        println!("size: {}", output.len());
    }

    #[test]
    fn fill_adv_data_test() {
        let mut data = [0u8; 31];
        let mut offset = 0;

        offset += fill_adv_data(&mut data, 0x01, &[0x03]);
        offset += fill_adv_data(&mut data[offset..], 0x09, &[b'A', b'F', b'O']);
        offset += fill_adv_data(&mut data[offset..], 0xff, &[0xff, 0xff]);

        #[rustfmt::skip]
        let correct_data = &[
            0x02, 0x01, 0x03,
            0x04, 0x09, b'A', b'F', b'O', 
            0x03, 0xff, 0xff, 0xff,
        ];

        assert_eq!(&data[..offset], &correct_data[..])
    }
}
