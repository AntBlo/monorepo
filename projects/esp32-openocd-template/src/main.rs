#![cfg_attr(not(test), no_std)]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_executor::Spawner;
use embassy_net::{
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
    Config, Stack, StackResources,
};
use embassy_time::{Duration, Timer};
use embedded_svc::wifi::{ClientConfiguration, Configuration, Wifi};
use esp32_hal::{
    clock::ClockControl,
    embassy::{self, executor::Executor},
    peripheral::Peripheral,
    peripherals::Peripherals,
    prelude::*,
    timer::TimerGroup,
    Rng,
};
use esp_println::println;
use esp_wifi::{
    initialize,
    wifi::{WifiApDevice, WifiController, WifiDevice, WifiEvent, WifiMode, WifiState},
    EspWifiInitFor,
};
use static_cell::make_static;

const SSID: &str = ""; // env!("SSID");
const PASSWORD: &str = ""; // env!("PASSWORD");

#[embassy_executor::task]
#[inline(never)]
async fn reconnect_task(mut controller: WifiController<'static>) {
    println!("Device capabilities: {:?}", controller.get_capabilities());
    loop {
        if let WifiState::StaConnected = esp_wifi::wifi::get_wifi_state() {
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await
        }

        if !controller.is_started().is_ok_and(|s| s) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start().await.unwrap();
            println!("Wifi started");
        }

        match controller.connect().await {
            Ok(_) => println!("Wifi connected!"),
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn run_stack(stack: &'static Stack<WifiDevice<'static, WifiApDevice>>) {
    stack.run().await
}

// const SSID: &str = env!("SSID");
// const PASSWORD: &str = env!("PASSWORD");

#[entry]
fn main() -> ! {
    println!("Init");
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::max(system.clock_control).freeze();

    let mut rng = Rng::new(peripherals.RNG);

    let timer_group1 = TimerGroup::new(peripherals.TIMG1, &clocks);

    let init: esp_wifi::EspWifiInitialization = initialize(
        EspWifiInitFor::Wifi,
        timer_group1.timer0,
        rng,
        system.radio_clock_control,
        &clocks,
    )
    .unwrap();

    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    embassy::init(&clocks, timer_group0);

    let wifi = peripherals.WIFI;

    let (wifi_interface, controller) =
        esp_wifi::wifi::new_with_mode(&init, wifi, WifiApDevice).unwrap();

    let config = Config::default();

    let a = (rng.random() as u64) << 32;
    let b = rng.random() as u64;
    let ab = a | b;
    let seed = ab;

    let stack = make_static!(Stack::new(
        wifi_interface,
        config,
        make_static!(StackResources::<3>::new()),
        seed
    ));

    let executor: &mut Executor = make_static!(Executor::new());

    executor.run(move |spawner| {
        spawner.spawn(reconnect_task(controller)).unwrap();
        spawner.spawn(run_stack(stack)).unwrap();
    });
}

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    esp_println::dbg!(panic_info);
    loop {}
}
