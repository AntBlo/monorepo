use embassy_executor::Spawner;
use embassy_net::driver::Driver;
use embassy_net::{Config, Ipv4Address, Ipv4Cidr, Stack, StackResources, StaticConfigV4};
use embassy_time::{Duration, Timer};
use embedded_svc::wifi::{ClientConfiguration, Configuration, Wifi};
use esp32_hal::clock::Clocks;
use esp32_hal::peripherals::WIFI;
use esp32_hal::system::RadioClockControl;
use esp32_hal::Rng;
use esp_println::println;
use esp_wifi::wifi::{
    new_with_mode, WifiController, WifiDevice, WifiEvent, WifiStaDevice, WifiState,
};
use esp_wifi::{initialize, EspWifiInitFor};
use static_cell::make_static;

use crate::env_variables::{PASSWORD, SSID};

type EspWifiTimer =
    esp32_hal::timer::Timer<esp32_hal::timer::Timer0<esp32_hal::peripherals::TIMG1>>;

pub async fn start_network_stack(
    spawner: Spawner,
    timer: EspWifiTimer,
    mut rng: Rng,
    radio_clock_control: RadioClockControl,
    clocks: &Clocks<'_>,
    wifi: WIFI,
) -> &'static Stack<impl Driver> {
    let init = initialize(
        EspWifiInitFor::Wifi,
        timer,
        rng,
        radio_clock_control,
        &clocks,
    )
    .unwrap();

    let (wifi_interface, controller) = new_with_mode(&init, wifi, WifiStaDevice).unwrap();

    let mac_address = wifi_interface.mac_address();
    println!(
        "MAC Address: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac_address[0],
        mac_address[1],
        mac_address[2],
        mac_address[3],
        mac_address[4],
        mac_address[5]
    );

    let device = Ipv4Address::new(192, 168, 1, 200);
    let netmask = Ipv4Address::new(255, 255, 255, 0);
    let address = Ipv4Cidr::from_netmask(device, netmask).unwrap();

    let gateway = Some(Ipv4Address::new(192, 168, 1, 1));
    let mut dns_servers = heapless::Vec::new();
    dns_servers.push(Ipv4Address::new(1, 1, 1, 1));

    let config = Config::ipv4_static(StaticConfigV4 {
        address,
        gateway,
        dns_servers,
    });

    let a = (rng.random() as u64) << 32;
    let b = rng.random() as u64;
    let seed = a | b;

    let stack = &*make_static!(Stack::new(
        wifi_interface,
        config,
        make_static!(StackResources::<3>::new()),
        seed
    ));

    spawner.spawn(connection(controller)).unwrap();
    spawner.spawn(net_task(&stack)).unwrap();

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    stack
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    println!("start connection task");
    println!("Device capabilities: {:?}", controller.get_capabilities());
    loop {
        match esp_wifi::wifi::get_wifi_state() {
            WifiState::StaConnected => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start().await.unwrap();
            println!("Wifi started!");
        }
        println!("About to connect...");

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
async fn net_task(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    stack.run().await
}
