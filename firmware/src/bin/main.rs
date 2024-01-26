#![no_std]
#![no_main]
#![macro_use]
#![feature(type_alias_impl_trait)]

use afo_fw::actors;
use afo_fw::actors::bluetooth::Server;

use afo_fw::actors::scd4x::{Scd4xData, Scd4xReader};
use afo_fw::actors::transformer::Transformer;

use afo_shared::AirQuality;
use embassy_nrf::gpio::{Input, Output, Pull};
use embassy_nrf::usb::SoftwareVbusDetect;
use sensirion_async::scd4x::Scd4x;
use static_cell::StaticCell;

use ector::{spawn_actor, ActorContext};
use embassy_nrf::twim::Twim;

use embassy_executor::Spawner;
use embassy_nrf::{interrupt, interrupt::InterruptExt};
use embassy_time::{Duration, Timer};

use afo_fw::actors::reactor::{self, Reactor};
use nrf_softdevice::Softdevice;

pub static USB_SUPPLY: StaticCell<SoftwareVbusDetect> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_nrf::init(afo_fw::embassy_config());
    #[cfg(feature = "panic-persist")]
    {
        if let Some(msg) = panic_persist::get_panic_message_bytes() {
            defmt::error!(
                "panic_raw: {} {:x}",
                unsafe { core::str::from_utf8_unchecked(msg) },
                msg
            );
        }
    }

    unsafe { afo_fw::reinitialize_reset() };

    let mut led = Output::new(
        p.P1_15,
        embassy_nrf::gpio::Level::Low,
        embassy_nrf::gpio::OutputDrive::Standard,
    );

    // defmt::error!(
    //     "err: {}",
    //     sensirion_async::Error::Bus(embassy_nrf::twim::Error::DataNack)
    // );
    // defmt::error!(
    //     "err: {}",
    //     sensirion_async::Error::<embassy_nrf::twim::Error>::Parsing(
    //         sensirion_async::ParsingError::Crc
    //     )
    // );

    // let flash_irq = interrupt::take!(QSPI);
    // flash_irq.set_priority(interrupt::Priority::P5);

    // defmt::info!("flashko");
    // let _qspi = ExternalFlashPins {
    //     qspi: p.QSPI,
    //     sck: p.P0_08,
    //     csn: p.P0_04,
    //     io0: p.P0_06,
    //     io1: p.P0_26,
    //     io2: p.P0_27,
    //     io3: p.P1_09,
    // }
    // .configure(flash_irq);

    // let flash = spawn_actor!(spawner, FLASH, Flash, Flash::new(qspi));

    // let blue_led = Output::new(p.P1_15.degrade(), Level::Low, OutputDrive::Standard);
    // let led_actor = actors::led::Led::new(blue_led);

    // let led = spawn_actor!(spawner, LED, actors::led::Led<Output<AnyPin>>, led_actor);

    // spawn_actor!(
    //     spawner,
    //     EMMITOR,
    //     Emitter<LedMessage>,
    //     Emitter::new(led, LedMessage::Toggle)
    // );

    let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
    irq.set_priority(interrupt::Priority::P2);
    let twi = Twim::new(p.TWISPI0, irq, p.P0_12, p.P0_13, Default::default());

    let scd40 = Scd4x::new(twi);

    static REACTOR: ActorContext<Reactor> = ActorContext::new();

    let reactor = REACTOR.address();

    let scd2reactor_transformer = spawn_actor!(spawner, SCD2REACTOR_TF, Transformer<Scd4xData, reactor::Message>, Transformer::new(reactor.clone()));

    spawn_actor!(
        spawner,
        SCD4X_READER,
        Scd4xReader,
        Scd4xReader::new(scd40, scd2reactor_transformer)
    );

    // let supply: &'static SignalledSupply = supply;

    // // // Create the driver, from the HAL.
    // let irq = interrupt::take!(USBD);
    // let driver = Driver::new(p.USBD, irq, supply);

    // // let usb = spawn_actor!(
    // //     spawner,
    // //     USB_SERIAL,
    // //     UsbSerial,
    // //     UsbSerial {
    // //         driver: Some(driver)
    // //     }
    // // );

    // let ui_reactor = spawn_actor!(
    //     spawner,
    //     BUTTON_REACTOR,
    //     UiReactor,
    //     UiReactor::new(display, REACTOR.address())
    // );

    let supply = USB_SUPPLY.init(SoftwareVbusDetect::new(true, true));

    let sd = Softdevice::enable(&afo_fw::softdevice_config());

    let server = defmt::unwrap!(Server::new(sd));

    spawner
        .spawn(actors::bluetooth::softdevice_task(sd, supply))
        .unwrap();

    let id_pin = Input::new(p.P0_28, Pull::Up);
    // First read was 0
    Timer::after(Duration::from_millis(100)).await;

    let device_id = if id_pin.is_low() { 0 } else { 1 };

    defmt::info!("Starting with device id: {}", device_id);

    let ble = spawn_actor!(
        spawner,
        BLE,
        actors::bluetooth::Ble,
        actors::bluetooth::Ble {
            softdevice: sd,
            server,
            air_quality: AirQuality::default(),
            device_id,
        }
    );

    REACTOR.mount(spawner, Reactor::new(Some(ble), None, None));

    loop {
        led.set_high();
        Timer::after(Duration::from_millis(50)).await;
        led.set_low();
        Timer::after(Duration::from_secs(60)).await;
    }
}
