#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use afo_fw::actors::bluetooth::Server;
use afo_fw::actors::emittor::Emitter;

use afo_fw::actors::usb_hid::UsbHid;
// use drogue_device::actors::button::Button;
use drogue_device::actors::transformer::Transformer;

use drogue_device::drivers::led::Led;
use drogue_device::drivers::{self, ActiveHigh, ActiveLow};

use drogue_device::drivers::led::neopixel::rgbw::NeoPixelRgbw;
use ector::{spawn_actor, ActorContext};
use embassy::blocking_mutex::raw::ThreadModeRawMutex;

use embassy::mutex::Mutex;
use embassy::util::Forever;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_nrf::gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin, Pull};
use embassy_nrf::twim::Twim;
use embassy_nrf::usb::{Driver, SignalledSupply};

use embassy::executor::Spawner;
use embassy::time::{Duration, Timer};
use embassy_nrf::{interrupt, interrupt::InterruptExt, peripherals, Peripherals};

use nrf_softdevice::Softdevice;
use sensirion_async::scd30::Scd30;
use sensirion_async::sgp40::Sgp40;
use sensirion_async::sps30::Sps30;

use afo_fw::actors::button::Button;
use afo_fw::actors::buzzer::Buzzer;
use afo_fw::actors::display::Display;
use afo_fw::actors::light_sound::LightSoundReactor;
use afo_fw::actors::neopixel::NeoPixel;
use afo_fw::actors::reactor::{self, Reactor};
use afo_fw::actors::scd30::{Scd30Data, Scd30Reader};
use afo_fw::actors::sgp40::Sgp40Reader;
use afo_fw::actors::sps30::Sps30Reader;
use afo_fw::actors::{self, EscPressed, NextPressed, OkPressed, PrevPressed, UiMessage, UiReactor};
use afo_fw::models::{AirQuality, Pm, Voc};

pub static USB_SUPPLY: Forever<SignalledSupply> = Forever::new();

// pub const EXTERNAL_FLASH_SIZE: usize = 2097152;
// pub const EXTERNAL_FLASH_BLOCK_SIZE: usize = 256;
// pub type ExternalFlash<'d> = qspi::Qspi<'d, peripherals::QSPI, EXTERNAL_FLASH_SIZE>;
// pub struct ExternalFlashPins {
//     pub qspi: peripherals::QSPI,
//     pub sck: peripherals::P0_19,
//     pub csn: peripherals::P0_20,
//     pub io0: peripherals::P0_17,
//     pub io1: peripherals::P0_22,
//     pub io2: peripherals::P0_23,
//     pub io3: peripherals::P0_21,
// }

// impl ExternalFlashPins {
//     pub fn configure<'d>(self) -> ExternalFlash<'d> {
//         let mut config = qspi::Config::default();
//         config.read_opcode = qspi::ReadOpcode::READ4IO;
//         config.write_opcode = qspi::WriteOpcode::PP4O;
//         config.write_page_size = qspi::WritePageSize::_256BYTES;
//         let irq = interrupt::take!(QSPI);
//         let mut q: qspi::Qspi<'_, _, EXTERNAL_FLASH_SIZE> = qspi::Qspi::new(
//             self.qspi, irq, self.sck, self.csn, self.io0, self.io1, self.io2, self.io3, config,
//         );

//         // Setup QSPI
//         let mut status = [4; 2];
//         q.blocking_custom_instruction(0x05, &[], &mut status[..1])
//             .unwrap();

//         q.blocking_custom_instruction(0x35, &[], &mut status[1..2])
//             .unwrap();

//         if status[1] & 0x02 == 0 {
//             status[1] |= 0x02;
//             q.blocking_custom_instruction(0x01, &status, &mut [])
//                 .unwrap();
//         }
//         q
//     }
// }

static SOFTDEVICE: Forever<Softdevice> = Forever::new();

#[embassy::main(config = "afo_fw::embassy_config()")]
async fn main(spawner: Spawner, p: Peripherals) {
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

    // let qspi = ExternalFlashPins {
    //     qspi: p.QSPI,
    //     sck: p.P0_19,
    //     csn: p.P0_20,
    //     io0: p.P0_17,
    //     io1: p.P0_22,
    //     io2: p.P0_23,
    //     io3: p.P0_21,
    // }
    // .configure();

    // let flash = spawn_actor!(spawner, FLASH, Flash, Flash::new(qspi));

    let supply = USB_SUPPLY.put(SignalledSupply::new(true, true));

    Timer::after(Duration::from_millis(2000)).await;

    let sd = Softdevice::enable(&afo_fw::softdevice_config());

    let server = defmt::unwrap!(Server::new(sd));

    spawner
        .spawn(actors::bluetooth::softdevice_task(sd, supply))
        .unwrap();

    let ble = spawn_actor!(
        spawner,
        BLE,
        actors::bluetooth::Ble,
        actors::bluetooth::Ble {
            softdevice: sd,
            server,
            air_quality: AirQuality::default()
        }
    );

    let blue_led = Output::new(p.P1_10.degrade(), Level::Low, OutputDrive::Standard);
    let led = Led::<_, ActiveHigh>::new(blue_led);
    let led_actor = drogue_device::actors::led::Led::new(led);

    let led = spawn_actor!(
        spawner,
        LED,
        drogue_device::actors::led::Led<Led<Output<AnyPin>>>,
        led_actor
    );

    spawn_actor!(spawner, EMMITOR, Emitter, Emitter::new(led));

    let buzzer = Output::new(p.P0_14.degrade(), Level::Low, OutputDrive::Standard);
    let buzzer = spawn_actor!(spawner, BUZZER, Buzzer, Buzzer::new(buzzer));

    let display_twi_irq = interrupt::take!(SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1);
    display_twi_irq.set_priority(interrupt::Priority::P2);
    let display_bus = Twim::new(
        p.TWISPI1,
        display_twi_irq,
        p.P1_08,
        p.P0_07,
        Default::default(),
    );

    let display = spawn_actor!(spawner, DISPLAY, Display, Display::new(display_bus));

    let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
    irq.set_priority(interrupt::Priority::P2);
    let twi = Twim::new(p.TWISPI0, irq, p.P0_12, p.P0_11, Default::default());

    static SHARED_BUS: Forever<Mutex<ThreadModeRawMutex, Twim<'static, peripherals::TWISPI0>>> =
        Forever::new();

    let shared_bus = SHARED_BUS.put(Mutex::new(twi));

    let scd = Scd30::new(I2cDevice::new(shared_bus));
    let sps = Sps30::new(I2cDevice::new(shared_bus));
    let sgp = Sgp40::new(I2cDevice::new(shared_bus));

    static REACTOR: ActorContext<Reactor> = ActorContext::new();

    let reactor = REACTOR.address();

    let scd2reactor_transformer = spawn_actor!(spawner, SCD2REACTOR_TF, Transformer<Scd30Data, reactor::Message>, Transformer::new(reactor.clone()));

    spawn_actor!(
        spawner,
        SCD30_READER,
        Scd30Reader,
        Scd30Reader::new(scd, scd2reactor_transformer)
    );

    let sps2reactor_transfomer = spawn_actor!(spawner, SPS2REACTOR_TF, Transformer<Pm, reactor::Message>, Transformer::new(reactor.clone()));

    spawn_actor!(
        spawner,
        SPS30_READER,
        Sps30Reader,
        Sps30Reader::new(sps, sps2reactor_transfomer)
    );

    let sgp2reactor_transformer = spawn_actor!(spawner, SGP2REACTOR_TF, Transformer<Voc, reactor::Message>, Transformer::new(reactor.clone()));

    let sgp = spawn_actor!(
        spawner,
        SGP40_READER,
        Sgp40Reader,
        Sgp40Reader::new(sgp, sgp2reactor_transformer)
    );

    let neopixel = defmt::unwrap!(NeoPixelRgbw::<'_, _, 1>::new(p.PWM0, p.P0_16));
    let neopixelko = spawn_actor!(spawner, NEOPIXEL, NeoPixel, NeoPixel { neopixel });

    let ls = spawn_actor!(
        spawner,
        LS_REACTOR,
        LightSoundReactor,
        LightSoundReactor::new(buzzer, neopixelko)
    );

    let supply: &'static SignalledSupply = supply;

    // // Create the driver, from the HAL.
    let irq = interrupt::take!(USBD);
    let driver = Driver::new(p.USBD, irq, supply);

    // let usb = spawn_actor!(
    //     spawner,
    //     USB_SERIAL,
    //     UsbSerial,
    //     UsbSerial {
    //         driver: Some(driver)
    //     }
    // );

    let usb = spawn_actor!(
        spawner,
        USB_SERIAL,
        UsbHid,
        UsbHid {
            driver: Some(driver)
        }
    );

    let button_reactor = spawn_actor!(
        spawner,
        BUTTON_REACTOR,
        UiReactor,
        UiReactor::new(display, REACTOR.address())
    );

    let airquality2ui_transformer = spawn_actor!(spawner, AIR2UI_TF, Transformer<AirQuality, UiMessage>, Transformer::new(button_reactor.clone()));

    REACTOR.mount(
        spawner,
        Reactor::new(sgp, airquality2ui_transformer, ls, usb, Some(ble), None),
    );

    let button: drivers::button::Button<Input<'static, AnyPin>, ActiveLow> =
        drivers::button::Button::new(Input::new(p.P0_13.degrade(), Pull::Up));
    let button_conversion_actor = spawn_actor!(spawner, ESC_TF, Transformer<EscPressed, UiMessage>, Transformer::new(button_reactor.clone()));
    let button_actor: Button<drivers::button::Button<Input<AnyPin>>, EscPressed> =
        Button::new(button, button_conversion_actor);

    spawn_actor!(
        spawner,
        ESC_BUTTON,
        Button<drivers::button::Button<Input<AnyPin>>, EscPressed>,
        button_actor
    );

    let button: drivers::button::Button<Input<'static, AnyPin>, ActiveLow> =
        drivers::button::Button::new(Input::new(p.P0_15.degrade(), Pull::Up));
    let button_conversion_actor = spawn_actor!(spawner, PREV_TF, Transformer<PrevPressed, UiMessage>, Transformer::new(button_reactor.clone()));
    let button_actor: Button<drivers::button::Button<Input<AnyPin>>, PrevPressed> =
        Button::new(button, button_conversion_actor);

    spawn_actor!(
        spawner,
        PREV_BUTTON,
        Button<drivers::button::Button<Input<AnyPin>>, PrevPressed>,
        button_actor
    );

    let button: drivers::button::Button<Input<'static, AnyPin>, ActiveLow> =
        drivers::button::Button::new(Input::new(p.P0_24.degrade(), Pull::Up));
    let button_conversion_actor = spawn_actor!(spawner, NEXT_TF, Transformer<NextPressed, UiMessage>, Transformer::new(button_reactor.clone()));
    let button_actor: Button<drivers::button::Button<Input<AnyPin>>, NextPressed> =
        Button::new(button, button_conversion_actor);

    spawn_actor!(
        spawner,
        NEXT_BUTTON,
        Button<drivers::button::Button<Input<AnyPin>>, NextPressed>,
        button_actor
    );

    let button: drivers::button::Button<Input<'static, AnyPin>, ActiveLow> =
        drivers::button::Button::new(Input::new(p.P0_25.degrade(), Pull::Up));
    let button_conversion_actor = spawn_actor!(spawner, OK_TF, Transformer<OkPressed, UiMessage>, Transformer::new(button_reactor.clone()));
    let button_actor: Button<drivers::button::Button<Input<AnyPin>>, OkPressed> =
        Button::new(button, button_conversion_actor);

    spawn_actor!(
        spawner,
        OK_BUTTON,
        Button<drivers::button::Button<Input<AnyPin>>, OkPressed>,
        button_actor
    );

    loop {
        Timer::after(Duration::from_millis(2000)).await;
    }
}
