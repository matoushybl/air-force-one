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
}

#[cfg(test)]
mod tests {
    use super::*;
    use postcard::{from_bytes_cobs, to_slice_cobs};

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
        };
        let mut buffer = [0u8; 100];
        let output = to_slice_cobs(&data, &mut buffer).unwrap();

        assert!(output.len() > 0);
        assert!(output.len() < 100);

        let data = from_bytes_cobs::<AirQuality>(output).unwrap();
        assert!(data.typical_particulate_matter_size > 1.2);
        assert!(data.typical_particulate_matter_size < 1.4);
    }
}
