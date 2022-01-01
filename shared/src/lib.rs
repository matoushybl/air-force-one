#![cfg_attr(not(test), no_std)]

#[cfg_attr(feature = "std", derive(Debug))]
#[cfg_attr(feature = "defmt_format", derive(defmt::Format))]
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Default, Clone, Copy)]
pub struct AirQuality {
    pub co2_concentration: f32,
    pub temperature: f32,
    pub humidity: f32,
    /// Mass Concentration PM1.0 [μg/m³]
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
    pub voc_index: u16,
}

#[cfg_attr(feature = "std", derive(Debug))]
#[cfg_attr(feature = "defmt_format", derive(defmt::Format))]
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Default, Clone, Copy)]
pub struct AirQualityAdvertisement {
    pub co2_concentration: u16,
    pub temperature: i16, // scaled by 0.1
    pub humidity: u8,
    pub mass_pm1_0: u16,
    pub mass_pm2_5: u16,
    pub mass_pm4_0: u16,
    pub mass_pm10: u16,
    pub voc_index: u16,
}

impl From<AirQuality> for AirQualityAdvertisement {
    fn from(raw: AirQuality) -> Self {
        AirQualityAdvertisement {
            co2_concentration: raw.co2_concentration as u16,
            temperature: (raw.temperature / 0.1) as i16,
            humidity: raw.humidity as u8,
            mass_pm1_0: raw.mass_pm1_0 as u16,
            mass_pm2_5: raw.mass_pm2_5 as u16,
            mass_pm4_0: raw.mass_pm4_0 as u16,
            mass_pm10: raw.mass_pm10 as u16,
            voc_index: raw.voc_index as u16,
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
    use postcard::{from_bytes_cobs, to_slice, to_slice_cobs};

    #[test]
    fn it_works() {
        let data = AirQuality {
            co2_concentration: 400.0,
            temperature: 22.5,
            humidity: 40.0,
            mass_pm1_0: 10.0,
            mass_pm2_5: 10.0,
            mass_pm4_0: 10.0,
            mass_pm10: 10.0,
            number_pm0_5: 2.0,
            number_pm1_0: 2.0,
            number_pm2_5: 2.0,
            number_pm4_0: 2.0,
            number_pm10: 2.0,
            typical_particulate_matter_size: 1.3,
            voc_index: 100,
        };
        let mut buffer = [0u8; 100];
        let output = to_slice_cobs(&data, &mut buffer).unwrap();

        println!("size: {}", output.len());

        assert!(output.len() > 0);
        assert!(output.len() < 100);

        let data = from_bytes_cobs::<AirQuality>(output).unwrap();
        assert!(data.typical_particulate_matter_size > 1.2);
        assert!(data.typical_particulate_matter_size < 1.4);
    }

    #[test]
    fn adv() {
        let data = AirQualityAdvertisement {
            co2_concentration: 400,
            temperature: 225,
            humidity: 40,
            mass_pm1_0: 10,
            mass_pm2_5: 10,
            mass_pm4_0: 10,
            mass_pm10: 10,
            voc_index: 100,
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
