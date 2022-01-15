use embassy::interrupt::InterruptExt;
use embassy::util::Forever;
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
use embassy_nrf::gpiote::InputChannel;
use embassy_nrf::usb::UsbBus;
use embassy_nrf::{interrupt, peripherals};
use futures_intrusive::sync::LocalMutex;
use nrf_softdevice::Softdevice;
use usb_device::class_prelude::UsbBusAllocator;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};

use embassy_hal_common::usb::{State, UsbSerial};
use embassy_nrf::peripherals::{TWISPI0, TWISPI1};
use embassy_nrf::twim::Twim;

use crate::{StaticSerialClassSet1, StaticUsb, Usb};

// led 1 - p1.15 - red
// led 2 - p1.10 - blue
// buzz - p0.14
// esc - p0.13
// prev - p0.15
// next - p0.24
// ok - p.025
pub struct Board<'a> {
    pub led: Output<'a, peripherals::P1_10>,
    pub buzzer: Output<'a, peripherals::P0_14>,
    pub esc_button: InputChannel<'a, peripherals::GPIOTE_CH0, peripherals::P0_13>,
    pub prev_button: InputChannel<'a, peripherals::GPIOTE_CH1, peripherals::P0_15>,
    pub next_button: InputChannel<'a, peripherals::GPIOTE_CH2, peripherals::P0_24>,
    pub ok_button: InputChannel<'a, peripherals::GPIOTE_CH3, peripherals::P0_25>,
    pub sensor_bus: &'static LocalMutex<Twim<'static, TWISPI0>>,
    pub display_bus: Twim<'a, TWISPI1>,
    pub usb: Usb,
    pub softdevice: &'a Softdevice,
}

impl<'a> Board<'a> {
    pub fn new(peripherals: embassy_nrf::Peripherals) -> Self {
        static SENSOR_BUS: Forever<LocalMutex<Twim<'static, TWISPI0>>> = Forever::new();
        static USB_READ_BUFFER: Forever<[u8; 128]> = Forever::new();
        static USB_WRITE_BUFFER: Forever<[u8; 128]> = Forever::new();
        static USB_ALLOCATOR: Forever<UsbBusAllocator<StaticUsb>> = Forever::new();
        static USB_STATE: Forever<
            State<'static, StaticUsb, StaticSerialClassSet1, interrupt::USBD>,
        > = Forever::new();

        let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
        irq.set_priority(interrupt::Priority::P2);
        let twi = Twim::new(
            peripherals.TWISPI0,
            irq,
            peripherals.P0_12,
            peripherals.P0_11,
            Default::default(),
        );

        let sensor_bus = LocalMutex::new(twi, true);

        let display_twi_irq = interrupt::take!(SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1);
        display_twi_irq.set_priority(interrupt::Priority::P2);
        let display_bus = Twim::new(
            peripherals.TWISPI1,
            display_twi_irq,
            peripherals.P1_08,
            peripherals.P0_07,
            Default::default(),
        );

        // usb
        let read_buf = USB_READ_BUFFER.put([0u8; 128]);
        let write_buf = USB_WRITE_BUFFER.put([0u8; 128]);

        let usb = USB_ALLOCATOR.put(UsbBus::new(peripherals.USBD));
        let serial = UsbSerial::new(usb, read_buf, write_buf);

        let device = UsbDeviceBuilder::new(usb, UsbVidPid(0x16c0, 0x27dd))
            .manufacturer("Fake company")
            .product("Serial port")
            .serial_number("AFO")
            .device_class(0x02)
            .build();

        let usb_irq = interrupt::take!(USBD);
        usb_irq.set_priority(interrupt::Priority::P2);

        let state = USB_STATE.put(embassy_hal_common::usb::State::new());
        let usb = unsafe { embassy_hal_common::usb::Usb::new(state, device, serial, usb_irq) };

        Self {
            led: Output::new(peripherals.P1_10, Level::Low, OutputDrive::Standard),
            buzzer: Output::new(peripherals.P0_14, Level::Low, OutputDrive::Standard),
            esc_button: InputChannel::new(
                peripherals.GPIOTE_CH0,
                Input::new(peripherals.P0_13, Pull::Up),
                embassy_nrf::gpiote::InputChannelPolarity::LoToHi,
            ),
            prev_button: InputChannel::new(
                peripherals.GPIOTE_CH1,
                Input::new(peripherals.P0_15, Pull::Up),
                embassy_nrf::gpiote::InputChannelPolarity::LoToHi,
            ),
            next_button: InputChannel::new(
                peripherals.GPIOTE_CH2,
                Input::new(peripherals.P0_24, Pull::Up),
                embassy_nrf::gpiote::InputChannelPolarity::LoToHi,
            ),
            ok_button: InputChannel::new(
                peripherals.GPIOTE_CH3,
                Input::new(peripherals.P0_25, Pull::Up),
                embassy_nrf::gpiote::InputChannelPolarity::LoToHi,
            ),
            sensor_bus: SENSOR_BUS.put(sensor_bus),
            display_bus,
            usb,
            softdevice: Softdevice::enable(&crate::softdevice_config()),
        }
    }
}
