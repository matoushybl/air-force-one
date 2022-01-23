// note: shameless port of the smartled-rs crate, modified to be as simple as possible and to work with embassy's spi implementation.

pub struct Color {
    red: u8,
    blue: u8,
    green: u8,
}

impl Color {
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, blue, green }
    }
}

pub mod colors {
    use super::Color;

    pub const MAX: u8 = 16;
    pub const RED: Color = Color::new(MAX, 0, 0);
    pub const GREEN: Color = Color::new(0, MAX, 0);
    pub const BLUE: Color = Color::new(0, 0, MAX);
    pub const ORANGE: Color = Color::new(MAX, MAX / 2, 0);
}

pub struct NeoPixel<T: embedded_hal::blocking::spi::Write<u8>> {
    bus: T,
}

impl<T> NeoPixel<T>
where
    T: embedded_hal::blocking::spi::Write<u8>,
{
    pub fn new(bus: T) -> NeoPixel<T> {
        Self { bus }
    }

    pub fn set_color(&mut self, color: Color) -> Result<(), T::Error> {
        let mut buffer = [0u8; 12];
        self.fill_with_byte(&mut buffer[..], color.green);
        self.fill_with_byte(&mut buffer[4..], color.red);
        self.fill_with_byte(&mut buffer[8..], color.blue);
        self.bus.write(&buffer)?;

        let flush_buffer = [0; 140];
        self.bus.write(&flush_buffer)?;
        Ok(())
    }

    fn fill_with_byte(&mut self, buffer: &mut [u8], mut data: u8) {
        // Send two bits in one spi byte. High time first, then the low time
        // The maximum for T0H is 500ns, the minimum for one bit 1063 ns.
        // These result in the upper and lower spi frequency limits
        let patterns = [0b1000_1000, 0b1000_1110, 0b11101000, 0b11101110];
        for i in 0..4 {
            let bits = (data & 0b1100_0000) >> 6;
            buffer[i] = patterns[bits as usize];
            data <<= 2;
        }
    }
}
