// pub mod filter {
//     use super::{InvalidChannel, Pixel};
//     use core::marker::PhantomData;

//     pub struct ComposedFilter<
//         P: Pixel<C>,
//         F1: Filter<P, C> + Sized,
//         F2: Filter<P, C> + Sized,
//         const C: usize,
//     > {
//         f1: F1,
//         f2: F2,
//         _marker: PhantomData<P>,
//     }

//     impl<P: Pixel<C>, F1: Filter<P, C> + Sized, F2: Filter<P, C> + Sized, const C: usize>
//         ComposedFilter<P, F1, F2, C>
//     {
//         fn new(f1: F1, f2: F2) -> Self {
//             Self {
//                 f1,
//                 f2,
//                 _marker: PhantomData,
//             }
//         }
//     }

//     impl<P: Pixel<C>, F1: Filter<P, C>, F2: Filter<P, C>, const C: usize> Filter<P, C>
//         for ComposedFilter<P, F1, F2, C>
//     {
//         fn apply(&self, pixel: &P) -> Result<P, InvalidChannel> {
//             let pixel = self.f1.apply(pixel)?;
//             self.f2.apply(&pixel)
//         }

//         fn complete(&mut self) {
//             self.f1.complete();
//             self.f2.complete();
//         }
//     }

//     pub trait Filter<P: Pixel<C>, const C: usize> {
//         fn apply(&self, pixel: &P) -> Result<P, InvalidChannel>;

//         fn complete(&mut self) {}

//         fn and<F: Filter<P, C> + Sized>(self, filter: F) -> ComposedFilter<P, Self, F, C>
//         where
//             Self: Sized,
//         {
//             ComposedFilter::new(self, filter)
//         }
//     }

//     // This table remaps linear input values
//     // (the numbers we’d like to use; e.g. 127 = half brightness)
//     // to nonlinear gamma-corrected output values
//     // (numbers producing the desired effect on the LED;
//     // e.g. 36 = half brightness).
//     const GAMMA8: [u8; 256] = [
//         0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1,
//         1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4,
//         4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10, 10, 11, 11, 11, 12, 12,
//         13, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 24,
//         24, 25, 25, 26, 27, 27, 28, 29, 29, 30, 31, 32, 32, 33, 34, 35, 35, 36, 37, 38, 39, 39, 40,
//         41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 50, 51, 52, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
//         64, 66, 67, 68, 69, 70, 72, 73, 74, 75, 77, 78, 79, 81, 82, 83, 85, 86, 87, 89, 90, 92, 93,
//         95, 96, 98, 99, 101, 102, 104, 105, 107, 109, 110, 112, 114, 115, 117, 119, 120, 122, 124,
//         126, 127, 129, 131, 133, 135, 137, 138, 140, 142, 144, 146, 148, 150, 152, 154, 156, 158,
//         160, 162, 164, 167, 169, 171, 173, 175, 177, 180, 182, 184, 186, 189, 191, 193, 196, 198,
//         200, 203, 205, 208, 210, 213, 215, 218, 220, 223, 225, 228, 231, 233, 236, 239, 241, 244,
//         247, 249, 252, 255,
//     ];

//     pub struct Gamma;

//     impl<P: Pixel<C>, const C: usize> Filter<P, C> for Gamma {
//         fn apply(&self, pixel: &P) -> Result<P, InvalidChannel> {
//             let mut filtered = pixel.clone();
//             for i in 0..C {
//                 filtered.set(i, GAMMA8[pixel.get(i)? as usize])?;
//             }
//             Ok(filtered)
//         }
//     }

//     pub struct Brightness(pub u8);

//     impl Brightness {
//         fn percent(mut percent: u8) -> Self {
//             if percent > 100 {
//                 percent = 100;
//             }

//             if percent == 0 {
//                 Self(0)
//             } else {
//                 Self((255 / percent) as u8)
//             }
//         }
//     }

//     impl<P: Pixel<C>, const C: usize> Filter<P, C> for Brightness {
//         fn apply(&self, pixel: &P) -> Result<P, InvalidChannel> {
//             let mut filtered = pixel.clone();
//             for i in 0..C {
//                 let val = pixel.get(i)?;
//                 let _filtered_val = (val as u16 * (self.0 as u16 + 1) / 256) as u8;
//                 filtered.set(i, (pixel.get(i)? as u16 * (self.0 as u16 + 1) / 256) as u8)?;
//             }
//             Ok(filtered)
//         }
//     }

//     pub enum CyclicDirection {
//         Up,
//         Down,
//     }

//     pub struct CyclicBrightness {
//         low: u8,
//         high: u8,
//         current: u8,
//         direction: CyclicDirection,
//         step_size: u8,
//     }

//     impl CyclicBrightness {
//         pub fn new(low: u8, high: u8, step_size: u8) -> Self {
//             Self {
//                 low,
//                 high,
//                 current: low,
//                 direction: CyclicDirection::Up,
//                 step_size,
//             }
//         }
//     }

//     impl<P: Pixel<C>, const C: usize> Filter<P, C> for CyclicBrightness {
//         fn apply(&self, pixel: &P) -> Result<P, InvalidChannel> {
//             Brightness(self.current).apply(pixel)
//         }

//         fn complete(&mut self) {
//             match self.direction {
//                 CyclicDirection::Up => {
//                     if self.current.saturating_add(self.step_size) >= self.high {
//                         self.current = self.high;
//                         self.direction = CyclicDirection::Down;
//                     } else {
//                         self.current += self.step_size;
//                     }
//                 }
//                 CyclicDirection::Down => {
//                     if self.current.saturating_sub(self.step_size) <= self.low {
//                         self.current = self.low;
//                         self.direction = CyclicDirection::Up;
//                     } else {
//                         self.current -= self.step_size;
//                     }
//                 }
//             }
//         }
//     }
// }

// #[allow(unused)]
// mod rgb {
//     use super::{filter::Filter, InvalidChannel, Pixel, RES};
//     use core::{
//         mem::transmute,
//         ops::{Add, Deref},
//         slice,
//     };
//     use embassy_nrf::Peripheral;
//     use embassy_nrf::{
//         gpio::Pin,
//         pwm::{
//             Config, Error, Instance, Prescaler, SequenceConfig, SequenceLoad, SequencePwm,
//             SingleSequenceMode, SingleSequencer,
//         },
//     };
//     use embassy_time::{Duration, Timer};

//     pub const BLACK: Rgb8 = Rgb8::new(0x00, 0x00, 0x00);
//     pub const WHITE: Rgb8 = Rgb8::new(0xFF, 0xFF, 0x0FF);
//     pub const RED: Rgb8 = Rgb8::new(0xFF, 0x00, 0x00);
//     pub const GREEN: Rgb8 = Rgb8::new(0x00, 0xFF, 0x00);
//     pub const BLUE: Rgb8 = Rgb8::new(0x00, 0x00, 0xFF);

//     #[cfg_attr(feature = "defmt", derive(defmt::Format))]
//     #[derive(Copy, Clone, PartialEq, Eq)]
//     pub struct Rgb8 {
//         r: u8,
//         g: u8,
//         b: u8,
//     }

//     impl Pixel<3> for Rgb8 {
//         fn get(&self, ch: usize) -> Result<u8, InvalidChannel> {
//             match ch {
//                 0 => Ok(self.g),
//                 1 => Ok(self.r),
//                 2 => Ok(self.b),
//                 _ => Err(InvalidChannel),
//             }
//         }

//         fn set(&mut self, ch: usize, val: u8) -> Result<(), InvalidChannel> {
//             match ch {
//                 0 => self.g = val,
//                 1 => self.r = val,
//                 2 => self.b = val,
//                 _ => Err(InvalidChannel)?,
//             }
//             Ok(())
//         }
//     }

//     impl Rgb8 {
//         pub const fn new(r: u8, g: u8, b: u8) -> Self {
//             Self { r, g, b }
//         }
//     }

//     impl Add for Rgb8 {
//         type Output = Rgb8;

//         fn add(self, rhs: Self) -> Self::Output {
//             Self::Output {
//                 r: self.r.saturating_add(rhs.r),
//                 g: self.g.saturating_add(rhs.g),
//                 b: self.b.saturating_add(rhs.b),
//             }
//         }
//     }

//     #[cfg_attr(feature = "defmt", derive(defmt::Format))]
//     #[repr(C)]
//     struct RawPwmRgb8<const N: usize> {
//         words: [[u16; 24]; N],
//         end: [u16; 40],
//     }

//     impl<const N: usize> RawPwmRgb8<N> {
//         pub fn from_iter<'i, I: Iterator<Item = &'i Rgb8>>(iter: I) -> Result<Self, Error> {
//             let mut raw = Self::default();
//             let mut cur = 0;
//             for (cur, color) in iter.enumerate() {
//                 if cur > N {
//                     return Err(Error::SequenceTooLong);
//                 }
//                 color.fill_pwm_words(&mut raw.words[cur]).ok().unwrap();
//             }
//             Ok(raw)
//         }
//     }

//     impl<const N: usize> Default for RawPwmRgb8<N> {
//         fn default() -> Self {
//             Self {
//                 words: [[0; 24]; N],
//                 end: [RES; 40],
//             }
//         }
//     }

//     impl<const N: usize> Into<RawPwmRgb8<N>> for &[Rgb8; N] {
//         fn into(self) -> RawPwmRgb8<N> {
//             let mut raw = RawPwmRgb8::default();
//             let mut cur = 0;
//             for color in self {
//                 color.fill_pwm_words(&mut raw.words[cur]).ok().unwrap();
//                 cur += 1;
//             }
//             raw
//         }
//     }

//     impl<const N: usize> Deref for RawPwmRgb8<N> {
//         type Target = [u16];

//         fn deref(&self) -> &Self::Target {
//             unsafe {
//                 let ptr: *const u16 = transmute(self as *const _ as *const u16);
//                 slice::from_raw_parts(ptr, (N * 24) + 40)
//             }
//         }
//     }

//     pub struct NeoPixelRgb<'d, T: Instance, const N: usize = 1> {
//         pwm: SequencePwm<'d, T>,
//     }

//     impl<'d, T: Instance, const N: usize> NeoPixelRgb<'d, T, N> {
//         pub fn new(
//             pwm: impl Peripheral<P = T> + 'd,
//             pin: impl Peripheral<P = impl Pin> + 'd,
//         ) -> Result<Self, Error> {
//             embassy_nrf::into_ref!(pwm);
//             embassy_nrf::into_ref!(pin);
//             let mut config = Config::default();
//             config.sequence_load = SequenceLoad::Common;
//             config.prescaler = Prescaler::Div1;
//             config.max_duty = 20; // 1.25us (1s / 16Mhz * 20)

//             Ok(Self {
//                 pwm: SequencePwm::new_1ch(pwm, pin, config)?,
//             })
//         }

//         pub async fn set(&mut self, pixels: &[Rgb8; N]) -> Result<(), Error> {
//             let mut seq_config = SequenceConfig::default();
//             seq_config.end_delay = 799;

//             let raw: RawPwmRgb8<N> = pixels.into();
//             let raw = &*raw;

//             let sequences = SingleSequencer::new(&mut self.pwm, &*raw, seq_config);
//             sequences.start(SingleSequenceMode::Times(1))?;

//             Timer::after(Duration::from_micros((30 * (N as u64 + 40)) + 100)).await;
//             Ok(())
//         }

//         pub async fn set_from_iter<'i, I: Iterator<Item = &'i Rgb8>>(
//             &mut self,
//             pixels: I,
//         ) -> Result<(), Error> {
//             let mut seq_config = SequenceConfig::default();
//             seq_config.end_delay = 799;

//             let raw = RawPwmRgb8::<N>::from_iter(pixels)?;
//             let raw = &*raw;

//             let sequences = SingleSequencer::new(&mut self.pwm, &*raw, seq_config);
//             sequences.start(SingleSequenceMode::Times(1))?;

//             Timer::after(Duration::from_micros((30 * (N as u64 + 40)) + 100)).await;
//             Ok(())
//         }

//         pub async fn set_with_filter<F: Filter<Rgb8, 3>>(
//             &mut self,
//             pixels: &[Rgb8; N],
//             filter: &mut F,
//         ) -> Result<(), Error> {
//             let mut filtered = [BLACK; N];
//             for (i, pixel) in pixels.iter().enumerate() {
//                 filtered[i] = filter
//                     .apply(pixel)
//                     .map_err(|_| Error::SequenceTimesAtLeastOne)?;
//             }
//             filter.complete();
//             self.set(&filtered).await
//         }
//     }
// }

// pub mod rgbw {
//     use super::{filter::Filter, InvalidChannel, Pixel, RES};
//     use core::{
//         mem::transmute,
//         ops::{Add, Deref},
//         slice,
//     };
//     use embassy_nrf::{
//         gpio::Pin,
//         pwm::{
//             Config, Error, Instance, Prescaler, SequenceConfig, SequenceLoad, SequencePwm,
//             SingleSequenceMode, SingleSequencer,
//         },
//     };
//     use embassy_nrf::{into_ref, Peripheral};
//     use embassy_time::{Duration, Timer};

//     pub const BLACK: Rgbw8 = Rgbw8::new(0x00, 0x00, 0x00, 0x00);
//     pub const WHITE: Rgbw8 = Rgbw8::new(0x00, 0x00, 0x000, 0xFF);
//     pub const RED: Rgbw8 = Rgbw8::new(0xFF, 0x00, 0x00, 0x00);
//     pub const GREEN: Rgbw8 = Rgbw8::new(0x00, 0xFF, 0x00, 0x00);
//     pub const BLUE: Rgbw8 = Rgbw8::new(0x00, 0x00, 0xFF, 0x00);

//     #[cfg_attr(feature = "defmt", derive(defmt::Format))]
//     #[derive(Copy, Clone, PartialEq, Eq)]
//     pub struct Rgbw8 {
//         r: u8,
//         g: u8,
//         b: u8,
//         w: u8,
//     }

//     impl Pixel<4> for Rgbw8 {
//         fn get(&self, ch: usize) -> Result<u8, InvalidChannel> {
//             match ch {
//                 0 => Ok(self.g),
//                 1 => Ok(self.r),
//                 2 => Ok(self.b),
//                 3 => Ok(self.w),
//                 _ => Err(InvalidChannel),
//             }
//         }

//         fn set(&mut self, ch: usize, val: u8) -> Result<(), InvalidChannel> {
//             match ch {
//                 0 => self.g = val,
//                 1 => self.r = val,
//                 2 => self.b = val,
//                 3 => self.w = val,
//                 _ => Err(InvalidChannel)?,
//             }
//             Ok(())
//         }
//     }

//     impl Rgbw8 {
//         pub const fn new(r: u8, g: u8, b: u8, w: u8) -> Self {
//             Self { r, g, b, w }
//         }
//     }

//     impl Add for Rgbw8 {
//         type Output = Rgbw8;

//         fn add(self, rhs: Self) -> Self::Output {
//             Self::Output {
//                 r: self.r.saturating_add(rhs.r),
//                 g: self.g.saturating_add(rhs.g),
//                 b: self.b.saturating_add(rhs.b),
//                 w: self.w.saturating_add(rhs.w),
//             }
//         }
//     }

//     #[cfg_attr(feature = "defmt", derive(defmt::Format))]
//     #[repr(C)]
//     struct RawPwmRgbw8<const N: usize> {
//         words: [[u16; 32]; N],
//         end: [u16; 40],
//     }

//     impl<const N: usize> RawPwmRgbw8<N> {
//         pub fn from_iter<'i, I: Iterator<Item = &'i Rgbw8>>(iter: I) -> Result<Self, Error> {
//             let mut raw = Self::default();
//             let mut cur = 0;
//             for color in iter {
//                 if cur > N {
//                     return Err(Error::SequenceTooLong);
//                 }
//                 color.fill_pwm_words(&mut raw.words[cur]).ok().unwrap();
//                 cur += 1;
//             }
//             Ok(raw)
//         }
//     }

//     impl<const N: usize> Default for RawPwmRgbw8<N> {
//         fn default() -> Self {
//             Self {
//                 words: [[0; 32]; N],
//                 end: [RES; 40],
//             }
//         }
//     }

//     impl<const N: usize> Into<RawPwmRgbw8<N>> for &[Rgbw8; N] {
//         fn into(self) -> RawPwmRgbw8<N> {
//             let mut raw = RawPwmRgbw8::default();
//             let mut cur = 0;
//             for color in self {
//                 color.fill_pwm_words(&mut raw.words[cur]).ok().unwrap();
//                 cur += 1;
//             }
//             raw
//         }
//     }

//     impl<const N: usize> Deref for RawPwmRgbw8<N> {
//         type Target = [u16];

//         fn deref(&self) -> &Self::Target {
//             unsafe {
//                 let ptr: *const u16 = transmute(self as *const _ as *const u16);
//                 slice::from_raw_parts(ptr, (N * 32) + 40)
//             }
//         }
//     }

//     pub struct NeoPixelRgbw<'d, T: Instance, const N: usize = 1> {
//         pwm: SequencePwm<'d, T>,
//     }

//     impl<'d, T: Instance, const N: usize> NeoPixelRgbw<'d, T, N> {
//         pub fn new(
//             pwm: impl Peripheral<P = T> + 'd,
//             pin: impl Peripheral<P = impl Pin> + 'd,
//         ) -> Result<Self, Error> {
//             into_ref!(pwm);
//             into_ref!(pin);
//             let mut config = Config::default();
//             config.sequence_load = SequenceLoad::Common;
//             config.prescaler = Prescaler::Div1;
//             config.max_duty = 20; // 1.25us (1s / 16Mhz * 20)

//             Ok(Self {
//                 pwm: SequencePwm::new_1ch(pwm, pin, config)?,
//             })
//         }

//         pub async fn set(&mut self, pixels: &[Rgbw8; N]) -> Result<(), Error> {
//             let mut seq_config = SequenceConfig::default();
//             seq_config.end_delay = 799;

//             let raw: RawPwmRgbw8<N> = pixels.into();
//             let raw = &*raw;

//             let sequences = SingleSequencer::new(&mut self.pwm, &*raw, seq_config);
//             sequences.start(SingleSequenceMode::Times(1))?;

//             Timer::after(Duration::from_micros((30 * (N as u64 + 40)) + 100)).await;
//             Ok(())
//         }

//         pub async fn set_from_iter<'i, I: Iterator<Item = &'i Rgbw8>>(
//             &mut self,
//             pixels: I,
//         ) -> Result<(), Error> {
//             let mut seq_config = SequenceConfig::default();
//             seq_config.end_delay = 799;

//             //let raw: RawPwmRgbw8<N> = pixels.into();
//             let raw = RawPwmRgbw8::<N>::from_iter(pixels)?;
//             let raw = &*raw;

//             let sequences = SingleSequencer::new(&mut self.pwm, &*raw, seq_config);
//             sequences.start(SingleSequenceMode::Times(1))?;

//             Timer::after(Duration::from_micros((30 * (N as u64 + 40)) + 100)).await;
//             Ok(())
//         }

//         pub async fn set_with_filter<F: Filter<Rgbw8, 4>>(
//             &mut self,
//             pixels: &[Rgbw8; N],
//             filter: &mut F,
//         ) -> Result<(), Error> {
//             let mut filtered = [BLACK; N];
//             for (i, pixel) in pixels.iter().enumerate() {
//                 filtered[i] = filter
//                     .apply(pixel)
//                     .map_err(|_| Error::SequenceTimesAtLeastOne)?;
//             }
//             filter.complete();
//             self.set(&filtered).await
//         }
//     }
// }

// const ONE: u16 = 0x8000 | 13;
// // Duty = 13/20 ticks (0.8us/1.25us) for a 1
// const ZERO: u16 = 0x8000 | 7;
// // Duty 7/20 ticks (0.4us/1.25us) for a 0
// const RES: u16 = 0x8000;

// pub struct InvalidChannel;

// pub trait Pixel<const N: usize>: Copy + Clone {
//     const CHANNELS: usize = N;

//     fn fill_pwm_words(&self, dst: &mut [u16]) -> Result<(), InvalidChannel> {
//         let mut cur = 0;
//         for i in 0..Self::CHANNELS {
//             let v = self.get(i)?;
//             Self::byte_to_word(v, &mut dst[cur..cur + 8]);
//             cur += 8;
//         }
//         Ok(())
//     }

//     fn byte_to_word(byte: u8, dst: &mut [u16]) {
//         let mut pos = 0;
//         let mut mask = 0x80;
//         for _ in 0..8 {
//             if (byte & mask) != 0 {
//                 dst[pos] = ONE;
//             } else {
//                 dst[pos] = ZERO;
//             }
//             pos += 1;
//             mask >>= 1;
//         }
//     }

//     fn get(&self, ch: usize) -> Result<u8, InvalidChannel>;
//     fn set(&mut self, ch: usize, val: u8) -> Result<(), InvalidChannel>;
// }
