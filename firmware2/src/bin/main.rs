#![no_std]
#![no_main]
#![macro_use]
#![feature(type_alias_impl_trait)]

use afo_fw::actors::bluetooth::Server;

use afo_fw::actors::scd4x::{Scd4xData, Scd4xReader};
use afo_fw::actors::transformer::Transformer;

use embassy_nrf::qspi;
use sensirion_async::scd4x::Scd4x;
use static_cell::StaticCell;

// use crate::actors::neopixel::rgbw::NeoPixelRgbw;
use ector::{spawn_actor, ActorContext};
use embassy_nrf::twim::Twim;
use embassy_nrf::usb::SignalledSupply;

use embassy_executor::Spawner;
use embassy_nrf::{interrupt, interrupt::InterruptExt, peripherals};
use embassy_time::{Duration, Timer};

use afo_fw::actors::reactor::{self, Reactor};
use nrf_softdevice::Softdevice;

use afo_fw::actors;
use afo_fw::models::AirQuality;

pub static USB_SUPPLY: StaticCell<SignalledSupply> = StaticCell::new();

pub const EXTERNAL_FLASH_SIZE: usize = 2097152;
pub const EXTERNAL_FLASH_BLOCK_SIZE: usize = 256;
pub type ExternalFlash<'d> = qspi::Qspi<'d, peripherals::QSPI, EXTERNAL_FLASH_SIZE>;

/// Pins for External QSPI flash
pub struct ExternalFlashPins {
    pub qspi: peripherals::QSPI,
    pub sck: peripherals::P0_08,
    pub csn: peripherals::P0_04,
    pub io0: peripherals::P0_06,
    pub io1: peripherals::P0_26,
    pub io2: peripherals::P0_27,
    pub io3: peripherals::P1_09,
}

impl ExternalFlashPins {
    /// Configure an external flash instance based on pins
    pub fn configure<'d>(self, irq: interrupt::QSPI) -> ExternalFlash<'d> {
        let mut config = qspi::Config::default();
        config.read_opcode = qspi::ReadOpcode::READ4IO;
        config.write_opcode = qspi::WriteOpcode::PP4O;
        config.write_page_size = qspi::WritePageSize::_256BYTES;
        let mut q: qspi::Qspi<'_, _, EXTERNAL_FLASH_SIZE> = qspi::Qspi::new(
            self.qspi, irq, self.sck, self.csn, self.io0, self.io1, self.io2, self.io3, config,
        );

        // Setup QSPI
        let mut status = [4; 2];
        q.blocking_custom_instruction(0x05, &[], &mut status[..1])
            .unwrap();

        q.blocking_custom_instruction(0x35, &[], &mut status[1..2])
            .unwrap();

        if status[1] & 0x02 == 0 {
            status[1] |= 0x02;
            q.blocking_custom_instruction(0x01, &status, &mut [])
                .unwrap();
        }
        q
    }
}

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

    let supply = USB_SUPPLY.init(SignalledSupply::new(true, true));

    Timer::after(Duration::from_millis(2000)).await;

    // let sd = Softdevice::enable(&afo_fw::softdevice_config());

    // let server = defmt::unwrap!(Server::new(sd));

    // spawner
    //     .spawn(actors::bluetooth::softdevice_task(sd, supply))
    //     .unwrap();

    // let ble = spawn_actor!(
    //     spawner,
    //     BLE,
    //     actors::bluetooth::Ble,
    //     actors::bluetooth::Ble {
    //         softdevice: sd,
    //         server,
    //         air_quality: AirQuality::default()
    //     }
    // );

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

    // // let usb = spawn_actor!(
    // //     spawner,
    // //     USB_SERIAL,
    // //     UsbHid,
    // //     UsbHid {
    // //         driver: Some(driver)
    // //     }
    // // );

    // let ui_reactor = spawn_actor!(
    //     spawner,
    //     BUTTON_REACTOR,
    //     UiReactor,
    //     UiReactor::new(display, REACTOR.address())
    // );

    // let airquality2ui_transformer = spawn_actor!(spawner, AIR2UI_TF, Transformer<AirQuality, UiMessage>, Transformer::new(ui_reactor.clone()));

    REACTOR.mount(spawner, Reactor::new(None, None, None));

    loop {
        Timer::after(Duration::from_millis(2000)).await;
    }
}
