#![cfg_attr(not(test), no_std)]
#![no_main]
#![feature(type_alias_impl_trait)]

mod env_variables;
mod heap_alloc;
mod motor;
mod start_network_stack;

use bit_field::BitField;
use core::str::from_utf8;
use embassy_executor::Spawner;
use embassy_net::tcp::{State, TcpSocket};
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::IpListenEndpoint;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Channel, Receiver};
use embassy_time::{Duration, Timer};
use embedded_io::{ReadReady, WriteReady};
use embedded_io_async::Write;
use esp32_hal::clock::ClockControl;
use esp32_hal::reset::software_reset;
use esp32_hal::timer::TimerGroup;
use esp32_hal::{embassy, peripherals::Peripherals, prelude::*};
use esp32_hal::{Rng, IO};
use esp_println::logger::init_logger;
use esp_println::println;
use heap_alloc::init_heap;
use log::{error, info, LevelFilter};
use start_network_stack::start_network_stack;
use static_cell::make_static;

use crate::motor::Motor;

#[main]
async fn main(spawner: Spawner) -> ! {
    init_heap();
    init_logger(LevelFilter::Info);

    info!("Logger works");

    let peripherals = Peripherals::take();

    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::max(system.clock_control).freeze();
    let rng = Rng::new(peripherals.RNG);
    let timer = TimerGroup::new(peripherals.TIMG1, &clocks).timer0;
    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    let wifi = peripherals.WIFI;
    let radio_clock_control = system.radio_clock_control;

    embassy::init(&clocks, timer_group0);

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    let mut motor1 = Motor::new(
        make_static!(io.pins.gpio23.into_push_pull_output()),
        make_static!(io.pins.gpio22.into_push_pull_output()),
        make_static!(io.pins.gpio21.into_push_pull_output()),
        make_static!(io.pins.gpio19.into_push_pull_output()),
    );

    let mut motor2 = Motor::new(
        make_static!(io.pins.gpio18.into_push_pull_output()),
        make_static!(io.pins.gpio5.into_push_pull_output()),
        make_static!(io.pins.gpio17.into_push_pull_output()),
        make_static!(io.pins.gpio16.into_push_pull_output()),
    );

    let stack = start_network_stack(spawner, timer, rng, radio_clock_control, &clocks, wifi).await;

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];

    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

    socket.set_timeout(None);
    // socket.set_keep_alive(None);
    // socket.set_timeout(Some(Duration::from_millis(10000)));
    socket.set_keep_alive(Some(Duration::from_millis(100)));

    let allow_address = IpListenEndpoint {
        addr: None,
        port: 80,
    };
    let mut command_buffer = [0u8; 1];
    let channel = make_static!(Channel::<NoopRawMutex, u8, 1>::new());
    let receiver = channel.receiver();

    spawner
        .spawn(motor_controller(motor1, motor2, receiver))
        .unwrap();

    loop {
        info!("Waiting for connection");
        socket.accept(allow_address).await.unwrap();
        info!("Got connection");

        loop {
            match socket.state() {
                State::Established => {}
                state => {
                    info!("{state}");
                    break;
                }
            }

            match socket.read_ready() {
                Ok(true) => {}
                Ok(false) => {
                    Timer::after(Duration::from_millis(1)).await;
                    continue;
                }
                Err(err) => {
                    info!("{err:?}");
                    break;
                }
            };

            let Ok(num_bytes_read) = socket.read(&mut command_buffer).await else {
                break;
            };

            if num_bytes_read == 0 {
                continue;
            }

            let command_byte = *command_buffer.first().unwrap();
            info!("{:08b}", command_byte);
            channel.send(command_byte).await;
        }

        let command_byte = 0b101u8;
        channel.send(command_byte).await;

        info!("aborted");
        socket.abort();
        if socket.flush().await.is_err() {
            error!("Flush failed");
            continue;
        };
    }
}

#[embassy_executor::task]
async fn motor_controller(
    mut motor1: Motor,
    mut motor2: Motor,
    receiver: Receiver<'static, NoopRawMutex, u8, 1>,
) {
    let mut command_byte = 0b101u8;
    loop {
        command_byte = receiver.try_receive().unwrap_or(command_byte);
        let center = command_byte.get_bit(0);
        let lean = command_byte.get_bit(1);
        let stop = command_byte.get_bit(2);
        let direction = command_byte.get_bit(3);

        match (center, lean) {
            (true, _) => {
                motor1.stop().await;
            }
            (_, true) => {
                info!("lean left");
                motor1.forward().await;
            }
            (_, false) => {
                info!("lean right");
                motor1.backward().await;
            }
        }

        match (stop, direction) {
            (true, _) => {
                motor2.stop().await;
            }
            (_, true) => {
                info!("stop forward");
                motor2.forward().await;
            }
            (_, false) => {
                info!("stop backward");
                motor2.backward().await;
            }
        }
    }
}

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    println!("{panic_info}");
    // software_reset();
    loop {}
}
