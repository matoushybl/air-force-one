#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use core::cell::RefCell;

use embassy_executor::Spawner;
use embassy_net::tcp::TcpSocket;
use embassy_net::{Config, Ipv4Address, StackResources};
use embassy_sync::blocking_mutex::NoopMutex;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_println as _;
use esp_println::println;
use esp_wifi::wifi::{ClientConfiguration, Configuration};
use hal::embassy;
use hal::i2c::I2C;
use hal::Rng;
use hal::{clock::ClockControl, peripherals::Peripherals, prelude::*, IO};
use rust_mqtt::client::client::MqttClient;
use rust_mqtt::utils::rng_generator::CountingRng;
use sensirion_async::scd4x::{Celsius, Meter, Scd4x};
use static_cell::make_static;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");
const SERVER_IP: &str = env!("SERVER_IP");

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    defmt::error!("panic: {:?}", defmt::Debug2Format(info));
    hal::reset::software_reset();
    loop {}
}

#[derive(Default, Debug, Clone)]
struct State {
    co2_concentration: u16,
    temperature: f32,
    humidity: f32,
}

#[main]
async fn main(spawner: Spawner) {
    defmt::info!("Hello world!");
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::max(system.clock_control).freeze();

    embassy::init(
        &clocks,
        hal::timer::TimerGroup::new(peripherals.TIMG0, &clocks),
    );
    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    let state = make_static!(embassy_sync::blocking_mutex::NoopMutex::new(RefCell::new(
        State::default()
    )));

    let i2c0 = I2C::new(
        peripherals.I2C0,
        io.pins.gpio8,
        io.pins.gpio10,
        100u32.kHz(),
        &clocks,
    );
    let scd40 = Scd4x::new(i2c0);
    spawner.spawn(scd4x_task(scd40, state)).unwrap();
    let mut rng = Rng::new(peripherals.RNG);
    let stack_seed = rng.random() as u64;

    let init = esp_wifi::initialize(
        esp_wifi::EspWifiInitFor::Wifi,
        hal::systimer::SystemTimer::new(peripherals.SYSTIMER).alarm0,
        rng,
        system.radio_clock_control,
        &clocks,
    )
    .unwrap();

    let (wifi_interface, controller) =
        esp_wifi::wifi::new_with_mode(&init, peripherals.WIFI, esp_wifi::wifi::WifiStaDevice)
            .unwrap();

    let stack = &*make_static!(embassy_net::Stack::new(
        wifi_interface,
        Config::dhcpv4(Default::default()),
        make_static!(StackResources::<3>::new()),
        stack_seed
    ));

    spawner.spawn(connection(controller)).ok();
    spawner.spawn(net_task(stack)).ok();

    wait_for_connection(stack).await;
    spawner.spawn(comm(stack, state)).ok();
}

#[embassy_executor::task]
async fn comm(
    stack: &'static embassy_net::Stack<
        esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>,
    >,
    state: &'static NoopMutex<RefCell<State>>,
) {
    use core::fmt::Write;
    let rx_buffer = make_static!([0; 512]);
    let tx_buffer = make_static!([0; 512]);

    let mut octets = [0u8; 4];
    for (idx, oct) in SERVER_IP
        .splitn(4, '.')
        .map(|s| s.parse::<u8>().unwrap())
        .enumerate()
    {
        octets[idx] = oct;
    }

    loop {
        let mut socket = TcpSocket::new(stack, rx_buffer, tx_buffer);
        if socket.connect((Ipv4Address(octets), 1883)).await.is_err() {
            defmt::error!("failed to connect to MQTT broker");
            Timer::after_secs(2).await;
            continue;
        }

        let mut config = rust_mqtt::client::client_config::ClientConfig::new(
            rust_mqtt::client::client_config::MqttVersion::MQTTv5,
            CountingRng(10),
        );
        config.add_max_subscribe_qos(rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS0);
        config.add_client_id("afo-c3");
        config.max_packet_size = 100;
        let mut recv_buffer = [0; 80];
        let mut write_buffer = [0; 80];

        let mut client =
            MqttClient::<_, 5, _>::new(socket, &mut write_buffer, 80, &mut recv_buffer, 80, config);

        if client.connect_to_broker().await.is_err() {
            defmt::error!("failed to connect to MQTT broker");
            Timer::after_secs(2).await;
            continue;
        }

        let topic = "afo-c3";
        let mut json = heapless::String::<64>::new();

        {
            let s = state.lock(|c| c.borrow().clone());
            write!(
                &mut json,
                r#"{{"co2": "{}", "temperature": "{:.1}", "humidity": "{}"}}"#,
                s.co2_concentration, s.temperature, s.humidity
            )
            .unwrap();
        }

        if client
            .send_message(
                topic,
                json.as_bytes(),
                rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS0,
                true,
            )
            .await
            .is_err()
        {
            println!("failed to send MQTT message");
            Timer::after_secs(2).await;
            continue;
        }

        // do not remove as the mqtt message will not be sent.
        // rust mqtt doesn't support flushing at the moment
        Timer::after_secs(2).await;
    }
}

#[embassy_executor::task]
async fn connection(mut controller: esp_wifi::wifi::WifiController<'static>) {
    loop {
        if esp_wifi::wifi::get_wifi_state() == esp_wifi::wifi::WifiState::StaConnected {
            // wait until we're no longer connected
            controller
                .wait_for_event(esp_wifi::wifi::WifiEvent::StaDisconnected)
            .await;
            Timer::after(Duration::from_millis(5000)).await
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            controller.start().await.unwrap();
        }

        match controller.connect().await {
            Ok(_) => defmt::info!("Wifi connected!"),
            Err(e) => {
                defmt::error!("Failed to connect to wifi: {:?}", e);
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(
    stack: &'static embassy_net::Stack<
        esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>,
    >,
) {
    stack.run().await
}

#[embassy_executor::task]
async fn scd4x_task(
    mut sensor: Scd4x<I2C<'static, hal::peripherals::I2C0>>,
    state: &'static NoopMutex<RefCell<State>>,
) {
    const ALTITUDE: Meter = Meter(230);
    const TEMPERATURE_OFFSET: Celsius = Celsius(2.5);

    sensor.stop_periodic_measurement().await.unwrap();
    Timer::after(Duration::from_millis(500)).await;

    let serial_number = sensor.read_serial_number().await.unwrap();
    defmt::info!("SCD4x serial number: {:x}", serial_number);

    let configured_altitude = sensor.get_sensor_altitude().await.unwrap();
    if configured_altitude != ALTITUDE {
        sensor.set_sensor_altitude(ALTITUDE).await.unwrap()
    }

    let configured_offset = sensor.get_temperature_offset().await.unwrap();
    if configured_offset != TEMPERATURE_OFFSET {
        sensor
            .set_temperature_offset(TEMPERATURE_OFFSET)
            .await
            .unwrap();
    }

    sensor.start_periodic_measurement().await.unwrap();
    Timer::after(Duration::from_millis(500)).await;

    loop {
        if sensor.data_ready().await.unwrap() {
            let measurement = sensor.read().await;
            match measurement {
                Ok(measurement) => {
                    state.lock(|c| {
                        let mut state = c.borrow_mut();
                        state.co2_concentration = measurement.co2;
                        state.humidity = measurement.humidity;
                        state.temperature = measurement.temperature;
                    });
                    defmt::info!(
                        "CO2: {}, Temperature: {}, Humidity: {}",
                        measurement.co2,
                        measurement.temperature,
                        measurement.humidity
                    );
                }
                Err(err) => {
                    defmt::error!("Error accessing Scd4x: {:?}", err);
                }
            }
        }

        Timer::after(Duration::from_secs(6)).await;
    }
}

async fn wait_for_connection(
    stack: &'static embassy_net::Stack<
        esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>,
    >,
) {
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    defmt::info!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            defmt::warn!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
}
