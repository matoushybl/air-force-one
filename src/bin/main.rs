#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use core::cell::Cell;

use air_force_one::{self as _, scd30::SCD30, sps30::Sps30};

use arrayvec::ArrayString;
use embassy::{
    blocking_mutex::{CriticalSectionMutex, Mutex},
    executor::Spawner,
    time::{Duration, Timer},
    util::Forever,
};
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    interrupt,
    peripherals::{P1_10, TWISPI0, TWISPI1},
    twim::{self, Twim},
    Peripherals,
};
use embedded_graphics::{
    drawable::Drawable,
    fonts::{Font6x8, Text},
    pixelcolor::BinaryColor,
    prelude::Point,
    style::TextStyleBuilder,
};
use embedded_hal::digital::v2::{OutputPin, StatefulOutputPin};
use futures_intrusive::sync::LocalMutex;
use ssd1306::{
    prelude::{DisplayRotation, DisplaySize128x32, GraphicsMode},
    Builder, I2CDIBuilder,
};

// led 1 - p1.15 - red
// led 2 - p1.10 - blue

// #[embassy::task]
async fn blinky_task(mut led: Output<'static, P1_10>) {
    loop {
        if defmt::unwrap!(led.is_set_high()) {
            defmt::unwrap!(led.set_low());
        } else {
            defmt::unwrap!(led.set_high());
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy::task]
async fn display_task(
    twim: Twim<'static, TWISPI1>,
    state: &'static CriticalSectionMutex<Cell<f32>>,
) {
    use core::fmt::Write;
    let interface = I2CDIBuilder::new().init(twim);
    let mut disp: GraphicsMode<_, _> = Builder::new()
        .size(DisplaySize128x32)
        .with_rotation(DisplayRotation::Rotate0)
        .connect(interface)
        .into();

    disp.init().unwrap();

    let text_style = TextStyleBuilder::new(Font6x8)
        .text_color(BinaryColor::On)
        .build();

    loop {
        disp.clear();

        let mut buf = ArrayString::<[_; 32]>::new();
        let data = state.lock(|data| data.get());
        write!(&mut buf, "{} ppm", data).unwrap();
        Text::new(&mut buf, Point::zero())
            .into_styled(text_style)
            .draw(&mut disp)
            .unwrap();

        disp.flush().unwrap();
        defmt::warn!("displaying {}", data);
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy::task]
async fn co2_task(
    mut sensor: SCD30<'static, Twim<'static, TWISPI0>>,
    state: &'static CriticalSectionMutex<Cell<f32>>,
) {
    loop {
        if sensor.get_data_ready().await.unwrap() {
            let measurement = sensor.read_measurement().await.unwrap();
            state.lock(|data| data.set(measurement.co2));
            defmt::info!("wtf: {}", measurement.co2);
        }
    }
}

#[embassy::task]
async fn pm_task(mut sensor: Sps30<'static, Twim<'static, TWISPI0>>) {
    let version = defmt::unwrap!(sensor.read_version().await);

    defmt::error!("version {:x}", version);

    defmt::unwrap!(sensor.start_measurement().await);

    Timer::after(Duration::from_millis(2000)).await;
    defmt::error!("tf");

    loop {
        Timer::after(Duration::from_millis(500)).await;
        let ready = defmt::unwrap!(sensor.is_ready().await);
        defmt::error!("rdy {:x}", ready);

        if ready {
            defmt::error!("dredy");

            let data = defmt::unwrap!(sensor.read_measured_data().await);
            defmt::error!("data: {}", data);
        }
    }
}

static SENSOR_BUS: Forever<LocalMutex<Twim<'static, TWISPI0>>> = Forever::new();
static STATE: Forever<CriticalSectionMutex<Cell<f32>>> = Forever::new();

#[embassy::main]
async fn main(spawner: Spawner, p: Peripherals) {
    defmt::info!("Hello World!");

    let led = Output::new(p.P1_10, Level::Low, OutputDrive::Standard);

    Timer::after(Duration::from_millis(500)).await;
    let config = twim::Config::default();
    let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
    let twi = Twim::new(p.TWISPI0, irq, p.P0_12, p.P0_11, config);

    let sensor_bus = LocalMutex::new(twi, true);

    let sensor_bus = SENSOR_BUS.put(sensor_bus);

    let co2_sensor = SCD30::init(sensor_bus);
    let pm_sensor = Sps30::new(sensor_bus);

    let display_twi_config = twim::Config::default();
    let display_twi_irq = interrupt::take!(SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1);
    let display_twi = Twim::new(
        p.TWISPI1,
        display_twi_irq,
        p.P1_08,
        p.P0_07,
        display_twi_config,
    );

    let state = STATE.put(CriticalSectionMutex::new(Cell::new(0.0f32)));
    defmt::unwrap!(spawner.spawn(co2_task(co2_sensor, state)));
    defmt::unwrap!(spawner.spawn(display_task(display_twi, state)));
    defmt::unwrap!(spawner.spawn(pm_task(pm_sensor)));
    blinky_task(led).await;
}
