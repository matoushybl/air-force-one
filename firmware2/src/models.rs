#[derive(Clone, Copy, Debug, Default, defmt::Format)]
pub struct Co2(pub f32);

#[derive(Clone, Copy, Debug, Default, defmt::Format)]
pub struct Temperature(pub f32);

#[derive(Clone, Copy, Debug, Default, defmt::Format)]
pub struct Humidity(pub f32);

#[derive(Clone, Copy, Debug, Default, defmt::Format)]
pub struct Voc {
    pub index: u16,
    pub raw: u16,
}

#[derive(Clone, Copy, Debug, Default, defmt::Format)]
pub struct TemperatureAndHumidity {
    pub temperature: Temperature,
    pub humidity: Humidity,
}

#[derive(Clone, Copy, Debug, Default, defmt::Format)]
pub struct Pm {
    pub mass_10: f32,
    pub mass_25: f32,
    pub mass_40: f32,
    pub mass_100: f32,
    pub average_particle_size: f32,
}

#[derive(Clone, Copy, Debug, Default, defmt::Format)]
pub struct AirQuality {
    pub co2: Co2,
    pub temperature: Temperature,
    pub humidity: Humidity,
    pub pm: Pm,
    pub voc: Voc,
}
