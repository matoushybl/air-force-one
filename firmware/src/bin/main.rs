#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use core::cell::Cell;

use air_force_one::{self as _, scd30::SCD30, sps30::Sps30};

use arrayvec::ArrayString;
use embassy::blocking_mutex::kind::Noop;
use embassy::channel::mpsc::{self, Channel, Receiver, Sender};
use embassy::{
    blocking_mutex::{CriticalSectionMutex, Mutex},
    executor::Spawner,
    time::{Duration, Timer},
    util::Forever,
};
use embassy_hal_common::usb::{
    usb_serial::{UsbSerial, USB_CLASS_CDC},
    ClassSet1, State,
};
use embassy_nrf::gpio::{Input, Pull};
use embassy_nrf::gpiote::InputChannel;
use embassy_nrf::peripherals::{
    GPIOTE_CH0, GPIOTE_CH1, GPIOTE_CH2, GPIOTE_CH3, P0_13, P0_14, P0_15, P0_24, P0_25,
};
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    interrupt,
    peripherals::{P1_10, TWISPI0, TWISPI1},
    twim::{self, Twim},
    usbd::UsbPeripheral,
    Peripherals,
};
use embedded_graphics::{
    drawable::Drawable,
    fonts::{Font6x8, Text},
    pixelcolor::BinaryColor,
    prelude::Point,
    style::TextStyleBuilder,
};
use embedded_hal::digital::v2::{InputPin, OutputPin, StatefulOutputPin};
use futures::future::{select, Either};
use futures::pin_mut;
use futures_intrusive::sync::LocalMutex;
use nrf_usbd::Usbd;
use shared::AirQuality;
use ssd1306::{
    prelude::{DisplayRotation, DisplaySize128x32, GraphicsMode},
    Builder, I2CDIBuilder,
};
use usb_device::{
    class_prelude::UsbBusAllocator,
    device::{UsbDeviceBuilder, UsbVidPid},
};

#[embassy::task]
async fn ui_task(
    mut receiver: Receiver<'static, Noop, ButtonEvent, 1>,
    page: &'static CriticalSectionMutex<Cell<Page>>,
) {
    loop {
        let sel = select(receiver.recv(), Timer::after(Duration::from_secs(10))).await;
        match sel {
            Either::Left((Some(event), _)) => {
                match event {
                    ButtonEvent::Esc => page.lock(|page| page.set(Page::Basic)),
                    ButtonEvent::Ok => defmt::error!("ok not implemented."),
                    ButtonEvent::Next => page.lock(|page| match page.get() {
                        Page::Basic => page.set(Page::Pm),
                        Page::Pm => page.set(Page::Voc),
                        Page::Voc => page.set(Page::Basic),
                    }),
                    ButtonEvent::Prev => page.lock(|page| match page.get() {
                        Page::Basic => page.set(Page::Voc),
                        Page::Pm => page.set(Page::Basic),
                        Page::Voc => page.set(Page::Pm),
                    }),
                }
                defmt::error!("button_event_received");
            }
            Either::Right(_) => {
                page.lock(|page| page.set(Page::Basic));
                defmt::error!("timeout")
            }
            _ => {}
        }
    }
}

#[embassy::task]
async fn process_data(
    state: &'static CriticalSectionMutex<Cell<AirQuality>>,
    mut buzz: Output<'static, P0_14>,
) {
    let mut count = 0;
    loop {
        Timer::after(Duration::from_secs(1)).await;
        if state.lock(|state| state.get().co2_concentration) > 1500.0 {
            count += 1;
        } else {
            count = 0;
        }

        if count >= 10 {
            buzz.set_high();
            Timer::after(Duration::from_millis(200)).await;
            buzz.set_low();
            count = 0;
        }
    }
}

#[embassy::task]
async fn display_task(
    twim: Twim<'static, TWISPI1>,
    state: &'static CriticalSectionMutex<Cell<AirQuality>>,
    page: &'static CriticalSectionMutex<Cell<Page>>,
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

        let mut buf = ArrayString::<[_; 64]>::new();
        let data = state.lock(|data| data.get());
        match page.lock(|data| data.get()) {
            Page::Basic => write!(
                &mut buf,
                "Temp: {:.1} C\nHumi: {:.1} %\nCO2:  {:.0} ppm",
                data.temperature, data.humidity, data.co2_concentration
            )
            .unwrap(),
            Page::Pm => write!(
                &mut buf,
                "1.0: {:.1} ug/m3\n2.5: {:.1} ug/m3\n4.0: {:.1} ug/m3\n10:  {:.1} ug/m3",
                data.mass_pm1_0, data.mass_pm2_5, data.mass_pm4_0, data.mass_pm10
            )
            .unwrap(),
            Page::Voc => write!(
                &mut buf,
                "size: {:.1}",
                data.typical_particulate_matter_size
            )
            .unwrap(),
        }
        Text::new(&mut buf, Point::zero())
            .into_styled(text_style)
            .draw(&mut disp)
            .unwrap();

        disp.flush().unwrap();
        defmt::info!("DISP: displaying {}", data);
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy::task]
async fn co2_task(
    mut sensor: SCD30<'static, Twim<'static, TWISPI0>>,
    state: &'static CriticalSectionMutex<Cell<AirQuality>>,
) {
    loop {
        if sensor.get_data_ready().await.unwrap() {
            let measurement = sensor.read_measurement().await.unwrap();
            state.lock(|data| {
                let mut raw = data.get();
                raw.co2_concentration = measurement.co2;
                raw.temperature = measurement.temperature;
                raw.humidity = measurement.humidity;
                data.set(raw)
            });
            defmt::info!("SCD30: co2 {}", measurement.co2);
        }
    }
}

#[embassy::task]
async fn pm_task(
    mut sensor: Sps30<'static, Twim<'static, TWISPI0>>,
    state: &'static CriticalSectionMutex<Cell<AirQuality>>,
) {
    let version = defmt::unwrap!(sensor.read_version().await);

    defmt::error!("SPS30: version {:x}", version);

    defmt::unwrap!(sensor.start_measurement().await);

    Timer::after(Duration::from_millis(2000)).await;

    loop {
        Timer::after(Duration::from_millis(500)).await;

        if defmt::unwrap!(sensor.is_ready().await) {
            let measured = defmt::unwrap!(sensor.read_measured_data().await);
            state.lock(|data| {
                let mut raw = data.get();
                raw.mass_pm1_0 = measured.mass_pm1_0;
                raw.mass_pm2_5 = measured.mass_pm2_5;
                raw.mass_pm4_0 = measured.mass_pm4_0;
                raw.mass_pm10 = measured.mass_pm10;
                raw.number_pm0_5 = measured.number_pm0_5;
                raw.number_pm1_0 = measured.number_pm1_0;
                raw.number_pm2_5 = measured.number_pm2_5;
                raw.number_pm4_0 = measured.number_pm4_0;
                raw.number_pm10 = measured.number_pm10;
                raw.typical_particulate_matter_size = measured.typical_size;
                data.set(raw)
            });
            defmt::info!("SPS30: data: {}", measured);
        }
    }
}

#[embassy::task]
async fn report_task(
    usb: embassy_hal_common::usb::Usb<
        'static,
        Usbd<UsbPeripheral<'static>>,
        ClassSet1<
            Usbd<UsbPeripheral<'static>>,
            UsbSerial<'static, 'static, Usbd<UsbPeripheral<'static>>>,
        >,
        interrupt::USBD,
    >,
    state: &'static CriticalSectionMutex<Cell<AirQuality>>,
) {
    use embassy::io::AsyncWriteExt;
    pin_mut!(usb);
    let (mut _read_interface, mut write_interface) = usb.as_ref().take_serial_0();
    let mut buffer = [0u8; 100];
    loop {
        Timer::after(Duration::from_millis(500)).await;
        let data = state.lock(|data| data.get());
        if let Ok(raw) = postcard::to_slice_cobs(&data, &mut buffer) {
            defmt::unwrap!(write_interface.write_all(&raw).await);
        } else {
            defmt::error!("failed to serialize the state to raw data.");
        }
    }
}

static SENSOR_BUS: Forever<LocalMutex<Twim<'static, TWISPI0>>> = Forever::new();
static USB_READ_BUFFER: Forever<[u8; 128]> = Forever::new();
static USB_WRITE_BUFFER: Forever<[u8; 128]> = Forever::new();
static USB_ALLOCATOR: Forever<UsbBusAllocator<Usbd<UsbPeripheral>>> = Forever::new();
static USB_STATE: Forever<
    State<
        'static,
        Usbd<UsbPeripheral<'static>>,
        ClassSet1<
            Usbd<UsbPeripheral<'static>>,
            UsbSerial<'static, 'static, Usbd<UsbPeripheral<'static>>>,
        >,
        interrupt::USBD,
    >,
> = Forever::new();
static STATE: Forever<CriticalSectionMutex<Cell<AirQuality>>> = Forever::new();
static PAGE: Forever<CriticalSectionMutex<Cell<Page>>> = Forever::new();
static BUTTON_EVENTS: Forever<Channel<Noop, ButtonEvent, 1>> = Forever::new();

// led 1 - p1.15 - red
// led 2 - p1.10 - blue
// buzz - p0.14
// esc - p0.13
// prev - p0.15
// next - p0.24
// ok - p.025

macro_rules! naive_debounce {
    ($name:ident) => {
        async {
            loop {
                $name.wait().await;
                Timer::after(Duration::from_millis(10)).await;
                if defmt::unwrap!($name.is_high()) {
                    break;
                }
            }
        }
    };
}

enum ButtonEvent {
    Esc,
    Ok,
    Next,
    Prev,
}

#[derive(Clone, Copy)]
enum Page {
    Basic,
    Pm,
    Voc,
}

#[embassy::task]
async fn button_handling_task(
    esc: InputChannel<'static, GPIOTE_CH0, P0_13>,
    prev: InputChannel<'static, GPIOTE_CH1, P0_15>,
    next: InputChannel<'static, GPIOTE_CH2, P0_24>,
    ok: InputChannel<'static, GPIOTE_CH3, P0_25>,
    sender: Sender<'static, Noop, ButtonEvent, 1>,
) {
    loop {
        let esc_fut = naive_debounce!(esc);
        let prev_fut = naive_debounce!(prev);
        let next_fut = naive_debounce!(next);
        let ok_fut = naive_debounce!(ok);

        pin_mut!(esc_fut);
        pin_mut!(prev_fut);
        pin_mut!(next_fut);
        pin_mut!(ok_fut);
        let esc_ok_fut = select(esc_fut, ok_fut);
        let prev_next_fut = select(prev_fut, next_fut);
        let res = select(esc_ok_fut, prev_next_fut).await;
        match res {
            Either::Left((Either::Left(_), _)) => {
                defmt::unwrap!(sender.send(ButtonEvent::Esc).await);
            }
            Either::Left((Either::Right(_), _)) => {
                defmt::unwrap!(sender.send(ButtonEvent::Ok).await);
            }
            Either::Right((Either::Left(_), _)) => {
                defmt::unwrap!(sender.send(ButtonEvent::Prev).await);
            }
            Either::Right((Either::Right(_), _)) => {
                defmt::unwrap!(sender.send(ButtonEvent::Next).await);
            }
        }
    }
}

#[embassy::main]
async fn main(spawner: Spawner, p: Peripherals) {
    defmt::info!("Hello World!");

    let mut led = Output::new(p.P1_10, Level::Low, OutputDrive::Standard);
    let buzz = Output::new(p.P0_14, Level::Low, OutputDrive::Standard);
    let esc = InputChannel::new(
        p.GPIOTE_CH0,
        Input::new(p.P0_13, Pull::Up),
        embassy_nrf::gpiote::InputChannelPolarity::LoToHi,
    );

    let prev = InputChannel::new(
        p.GPIOTE_CH1,
        Input::new(p.P0_15, Pull::Up),
        embassy_nrf::gpiote::InputChannelPolarity::LoToHi,
    );

    let next = InputChannel::new(
        p.GPIOTE_CH2,
        Input::new(p.P0_24, Pull::Up),
        embassy_nrf::gpiote::InputChannelPolarity::LoToHi,
    );
    let ok = InputChannel::new(
        p.GPIOTE_CH3,
        Input::new(p.P0_25, Pull::Up),
        embassy_nrf::gpiote::InputChannelPolarity::LoToHi,
    );

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

    let bus = USB_ALLOCATOR.put(Usbd::new(UsbPeripheral::new(p.USBD)));

    defmt::error!("Hello world");

    let read_buf = USB_READ_BUFFER.put([0u8; 128]);
    let write_buf = USB_WRITE_BUFFER.put([0u8; 128]);
    let serial = UsbSerial::new(bus, read_buf, write_buf);

    let device = UsbDeviceBuilder::new(bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("Fake company")
        .product("Serial port")
        .serial_number("AFO")
        .device_class(USB_CLASS_CDC)
        .build();

    let state = USB_STATE.put(embassy_hal_common::usb::State::new());

    // sprinkle some unsafe

    let usb =
        unsafe { embassy_hal_common::usb::Usb::new(state, device, serial, interrupt::take!(USBD)) };

    unsafe {
        (*embassy_nrf::pac::USBD::ptr()).intenset.write(|w| {
            w.sof().set_bit();
            w.usbevent().set_bit();
            w.ep0datadone().set_bit();
            w.ep0setup().set_bit();
            w.usbreset().set_bit()
        })
    };

    let channel = BUTTON_EVENTS.put(Channel::new());
    let (sender, receiver) = mpsc::split(channel);
    let state = STATE.put(CriticalSectionMutex::new(Cell::new(AirQuality::default())));
    let page = PAGE.put(CriticalSectionMutex::new(Cell::new(Page::Basic)));
    defmt::unwrap!(spawner.spawn(co2_task(co2_sensor, state)));
    defmt::unwrap!(spawner.spawn(display_task(display_twi, state, page)));
    defmt::unwrap!(spawner.spawn(pm_task(pm_sensor, state)));
    defmt::unwrap!(spawner.spawn(report_task(usb, state)));
    defmt::unwrap!(spawner.spawn(button_handling_task(esc, prev, next, ok, sender)));
    defmt::unwrap!(spawner.spawn(ui_task(receiver, page)));
    defmt::unwrap!(spawner.spawn(process_data(state, buzz)));

    loop {
        if defmt::unwrap!(led.is_set_high()) {
            defmt::unwrap!(led.set_low());
        } else {
            defmt::unwrap!(led.set_high());
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}
