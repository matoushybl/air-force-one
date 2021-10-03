#![no_main]
#![no_std]

use air_force_one as _;
use smart_leds::RGB8; // global logger + panicking-behavior + memory layout

#[rtic::app(device = nrf52840_hal::pac, peripherals = true, dispatchers = [QSPI])]
mod app {
    use crate::RGB8Ext;
    use air_force_one::scd30::SCD30;
    use choreographer::colors::RED;
    use choreographer::engine::Sequence;
    use dwt_systick_monotonic::DwtSystick;
    use nrf52840_hal as hal;
    use nrf52840_hal::gpio::{Level, Output, Pin, PushPull};
    use nrf52840_hal::pac::PWM0;
    use nrf52840_hal::pac::SPI0;
    use nrf52840_hal::prelude::*;
    use nrf52840_hal::Twim;
    use nrf_smartled::pwm::Pwm;
    use rtic::rtic_monotonic::Milliseconds;
    use rtic::time::duration::Seconds;
    use smart_leds::colors::{BLUE, GREEN};
    use smart_leds::{SmartLedsWrite, RGB8};
    use ws2812_spi::Ws2812;

    type Neopixel = Ws2812<hal::spi::Spi<SPI0>>;
    type NeopixelPwm = Pwm<PWM0>;

    const MONO_HZ: u32 = 64_000_000; // 64 MHz

    #[monotonic(binds = SysTick, default = true)]
    type MyMono = DwtSystick<MONO_HZ>;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        led1: Pin<Output<PushPull>>,
        led2: Pin<Output<PushPull>>,
        neopixel: NeopixelPwm,
        sequence: Sequence<groundhog_nrf52::GlobalRollingTimer, 8>,
        co2_sensor: SCD30<Twim<hal::pac::TWIM0>>,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        let core: cortex_m::Peripherals = cx.core;
        let mut dcb = core.DCB;
        let dwt = core.DWT;
        let systick = core.SYST;
        let device: nrf52840_hal::pac::Peripherals = cx.device;

        // let _clocks = nrf52840_hal::clocks::Clocks::new(device.CLOCK);

        let p0 = hal::gpio::p0::Parts::new(device.P0);
        let p1 = hal::gpio::p1::Parts::new(device.P1);
        let mut led1 = p1.p1_15.into_push_pull_output(Level::Low).degrade();
        let mut led2 = p1.p1_10.into_push_pull_output(Level::Low).degrade();
        let neopixel_mosi = p0.p0_16.into_push_pull_output(Level::Low).degrade();

        let scl = p0.p0_11.into_floating_input().degrade();
        let sda = p0.p0_12.into_floating_input().degrade();

        let i2c = Twim::new(
            device.TWIM0,
            hal::twim::Pins { scl, sda },
            hal::twim::Frequency::K100,
        );

        groundhog_nrf52::GlobalRollingTimer::init(device.TIMER0);

        let mut scd = SCD30::init(i2c);

        groundhog_nrf52::GlobalRollingTimer::new().delay_ms(2000u32);
        let version = scd.read_fw_version().unwrap();

        defmt::error!("Received version is {:x} {:x}", version[0], version[1]);

        groundhog_nrf52::GlobalRollingTimer::new().delay_ms(2000u32);
        scd.set_measurement_interval(2);

        groundhog_nrf52::GlobalRollingTimer::new().delay_ms(2000u32);
        scd.start_continuous_measurement(0).unwrap();

        // let pwm = hal::pwm::Pwm::new();
        let neopixel = Pwm::new(device.PWM0, neopixel_mosi);
        // let neopixel_sck = p0.p0_14.into_push_pull_output(Level::Low).degrade();
        //
        // let pins = hal::spi::Pins {
        //     sck: neopixel_sck,
        //     mosi: neopixel_mosi.into(),
        //     miso: None
        // };
        //
        // let spi = hal::spi::Spi::new(device.SPI0, pins, hal::spi::Frequency::M2, hal::spi::MODE_0);
        // let neopixel = Ws2812::new(spi);

        let mono = DwtSystick::new(&mut dcb, dwt, systick, 64_000_000);

        led1.set_high();
        led2.set_high();
        for _ in 0..MONO_HZ / 64 {
            cortex_m::asm::nop();
        }

        led1.set_low();
        led2.set_low();

        let mut script: Sequence<groundhog_nrf52::GlobalRollingTimer, 8> = Sequence::empty();

        let red = RED.darken(0.1);
        let green = GREEN.darken(0.1);
        let blue = BLUE.darken(0.1);
        script.set(
            &choreographer::script! {
                | action |  color | duration_ms | period_ms_f | phase_offset_ms | repeat |
                |  sin   |    red |        2500 |      2500.0 |               0 |   once |
                |  sin   |  green |        2500 |      2500.0 |               0 |   once |
                |  sin   |   blue |        2500 |      2500.0 |               0 |   once |
            },
            choreographer::engine::LoopBehavior::LoopForever,
        );

        bar::spawn_after(Seconds(1_u32)).ok();
        neopixelize::spawn_after(Seconds(1_u32)).ok();
        // let p1 = device.az()
        (
            Shared {},
            Local {
                led1,
                led2,
                neopixel,
                sequence: script,
                co2_sensor: scd,
            },
            init::Monotonics(mono),
        )
    }

    #[idle(local = [led2])]
    fn idle(cx: idle::Context) -> ! {
        let led2: &mut Pin<Output<PushPull>> = cx.local.led2;
        loop {
            if led2.is_set_low().unwrap_or_default() {
                led2.set_high();
            } else {
                led2.set_low();
            }
        }
    }

    #[task(local = [led1, co2_sensor])]
    fn bar(cx: bar::Context) {
        let led1: &mut Pin<Output<PushPull>> = cx.local.led1;
        let sensor: &mut SCD30<Twim<hal::pac::TWIM0>> = cx.local.co2_sensor;

        if sensor.get_data_ready().unwrap() {
            let measurement = sensor.read_measurement().unwrap();
            defmt::error!("hello {}.", measurement.co2);
        }
        if led1.is_set_low().unwrap_or_default() {
            led1.set_high();
        } else {
            led1.set_low();
        }
        bar::spawn_after(Seconds(1_u32)).ok();
    }

    #[task(local = [neopixel, sequence])]
    fn neopixelize(cx: neopixelize::Context) {
        let neopixel: &mut NeopixelPwm = cx.local.neopixel;
        let script: &mut Sequence<groundhog_nrf52::GlobalRollingTimer, 8> = cx.local.sequence;
        if let Some(color) = script.poll() {
            neopixel.write([color].iter().cloned());
        }
        neopixelize::spawn_after(Milliseconds(10_u32)).ok();
    }
}

trait RGB8Ext {
    fn darken(self, ratio: f32) -> Self;
}

impl RGB8Ext for RGB8 {
    fn darken(self, ratio: f32) -> Self {
        Self {
            r: (self.r as f32 * ratio) as u8,
            g: (self.g as f32 * ratio) as u8,
            b: (self.b as f32 * ratio) as u8,
        }
    }
}
