use core::cell::Cell;

use embassy::blocking_mutex::kind::Noop;
use embassy::blocking_mutex::{CriticalSectionMutex, Mutex};
use embassy::channel::mpsc::{self, Channel};
use embassy::time::{Duration, Timer};
use embassy::util::Forever;
use shared::AirQuality;

use crate::board::Board;
use crate::drivers::sps30::PMInfo;
use crate::tasks;

#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    Basic,
    Pm,
    Voc,
    Settings,
}

impl Page {
    pub fn next(&self) -> Self {
        match self {
            Self::Basic => Self::Pm,
            Self::Pm => Self::Voc,
            Self::Voc => Self::Settings,
            Self::Settings => Self::Basic,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Page::Basic => Page::Settings,
            Page::Pm => Page::Basic,
            Page::Voc => Page::Pm,
            Page::Settings => Page::Voc,
        }
    }
}

pub enum ButtonEvent {
    Esc,
    Ok,
    Next,
    Prev,
}

#[derive(Clone, Copy)]
pub struct State {
    air_quality: AirQuality,
    page: Page,
    bzzz_enabled: bool,
}

#[derive(Clone, Copy)]
pub struct App {
    state: &'static CriticalSectionMutex<Cell<State>>,
}

impl App {
    pub async fn run(spawner: embassy::executor::Spawner, peripherals: embassy_nrf::Peripherals) {
        static STATE: Forever<CriticalSectionMutex<Cell<State>>> = Forever::new();

        static BUTTON_EVENTS: Forever<Channel<Noop, ButtonEvent, 1>> = Forever::new();
        let channel = BUTTON_EVENTS.put(Channel::new());
        let (sender, receiver) = mpsc::split(channel);

        let app = App {
            state: STATE.put(CriticalSectionMutex::new(Cell::new(State {
                air_quality: Default::default(),
                bzzz_enabled: true,
                page: Page::Basic,
            }))),
        };

        let board = Board::new(peripherals);

        defmt::unwrap!(spawner.spawn(tasks::sensors::co2_task(board.scd30, app)));
        defmt::unwrap!(spawner.spawn(tasks::sensors::pm_task(board.sps30, app)));
        defmt::unwrap!(spawner.spawn(tasks::sensors::voc_task(board.sgp40, app)));
        defmt::unwrap!(spawner.spawn(tasks::display::render(board.display_bus, app)));
        defmt::unwrap!(spawner.spawn(tasks::display::navigation(receiver, app)));
        defmt::unwrap!(spawner.spawn(tasks::usb::communication(board.usb, app)));
        defmt::unwrap!(spawner.spawn(tasks::buttons::task(
            board.esc_button,
            board.prev_button,
            board.next_button,
            board.ok_button,
            sender
        )));
        defmt::unwrap!(spawner.spawn(tasks::reporting::task(app, board.buzzer, board.neopixel)));
        defmt::unwrap!(spawner.spawn(tasks::bluetooth::softdevice_task(board.softdevice)));
        defmt::unwrap!(spawner.spawn(tasks::bluetooth::bluetooth_task(board.softdevice, app)));

        let mut led = board.led;
        loop {
            if led.is_set_high() {
                led.set_low();
                Timer::after(Duration::from_secs(3)).await;
            } else {
                led.set_high();
                Timer::after(Duration::from_millis(10)).await;
            }
        }
    }

    pub fn update_co2(&self, co2: f32, temperature: f32, humidity: f32) {
        self.state.lock(|c| {
            c.update(|mut s| {
                s.air_quality.co2_concentration = co2;
                s.air_quality.temperature = temperature;
                s.air_quality.humidity = humidity;
                s
            })
        });
    }

    pub fn update_pm(&self, air_info: PMInfo) {
        self.state.lock(|c| {
            c.update(|mut state| {
                state.air_quality.mass_pm1_0 = air_info.mass_pm1_0;
                state.air_quality.mass_pm2_5 = air_info.mass_pm2_5;
                state.air_quality.mass_pm4_0 = air_info.mass_pm4_0;
                state.air_quality.mass_pm10 = air_info.mass_pm10;
                state.air_quality.number_pm0_5 = air_info.number_pm0_5;
                state.air_quality.number_pm1_0 = air_info.number_pm1_0;
                state.air_quality.number_pm2_5 = air_info.number_pm2_5;
                state.air_quality.number_pm4_0 = air_info.number_pm4_0;
                state.air_quality.number_pm10 = air_info.number_pm10;
                state.air_quality.typical_particulate_matter_size = air_info.typical_size;
                state
            })
        });
    }

    pub fn update_voc(&self, voc: u16) {
        self.state.lock(|c| {
            c.update(|mut s| {
                s.air_quality.voc_index = voc;
                s
            })
        });
    }

    pub fn air_quality(&self) -> AirQuality {
        self.state.lock(|c| c.get().air_quality)
    }

    pub fn page(&self) -> Page {
        self.state.lock(|c| c.get().page)
    }

    fn set_page(&self, page: Page) {
        self.state.lock(|c| {
            c.update(|mut s| {
                s.page = page;
                s
            })
        });
    }

    pub fn button_pressed(&self, button: ButtonEvent) {
        match button {
            ButtonEvent::Esc => self.set_page(Page::Basic),
            ButtonEvent::Ok => {
                if self.page() == Page::Settings {
                    // TODO maybe save to persistent storage
                    self.set_bzzz(!self.bzzz_enabled());
                }
            }
            ButtonEvent::Next => {
                self.set_page(self.page().next());
            }
            ButtonEvent::Prev => {
                self.set_page(self.page().prev());
            }
        }
    }

    pub fn button_timed_out(&self) {
        self.set_page(Page::Basic);
    }

    pub fn bzzz_enabled(&self) -> bool {
        self.state.lock(|c| c.get().bzzz_enabled)
    }

    pub fn set_bzzz(&self, enabled: bool) {
        self.state.lock(|c| {
            c.update(|mut s| {
                s.bzzz_enabled = enabled;
                s
            })
        });
    }
}
