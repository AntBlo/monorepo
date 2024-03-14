mod create_server;
mod serial;
mod storage;

use std::{
    ops::DerefMut,
    sync::{Arc, Mutex},
    thread,
    time::{self, Duration},
};

use embedded_svc::http::{Headers, Method};
use enumset::enum_set;
use esp_idf_hal::{
    cpu::Core,
    delay::FreeRtos,
    gpio::PinDriver,
    prelude::Peripherals,
    task::watchdog::{TWDTConfig, TWDTDriver},
};

use create_server::create_server;
use esp_idf_svc::http::server::EspHttpServer;
use log::{error, info, Level, LevelFilter, Metadata, Record};
use serial::{create_serial, SerialWrapper};
use storage::{create_storage, BlockDev, StorageWrapper};

fn main() {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::set_max_level(LevelFilter::Trace);

    let mut peripherals = Peripherals::take().expect("Should get peripherals");
    let config =
        esp_idf_hal::uart::config::Config::default().baudrate(esp_idf_hal::units::Hertz(115_200));

    let mut serial = create_serial(
        peripherals.uart1,
        peripherals.pins.gpio6,
        peripherals.pins.gpio7,
        &config,
    );

    let config = TWDTConfig {
        duration: Duration::from_secs(10),
        panic_on_trigger: true,
        subscribed_idle_tasks: enum_set!(Core::Core0),
    };

    let mut driver = TWDTDriver::new(peripherals.twdt, &config).unwrap();

    {
        let mut watchdog = driver.watch_current_task().unwrap();

        serial.write("G28 ;Home\n", &mut watchdog).unwrap();
        serial
            .write("M106 S0 ; turn off cooling fan\n", &mut watchdog)
            .unwrap();
        serial
            .write("M104 S0 ; turn off extruder\n", &mut watchdog)
            .unwrap();
        serial
            .write("M140 S0 ; turn off bed\n", &mut watchdog)
            .unwrap();
        serial
            .write("M84 ; disable motors\n", &mut watchdog)
            .unwrap();
        serial
            .write("M82 ;absolute extrusion mode\n", &mut watchdog)
            .unwrap();

        serial.clear().unwrap();
    }

    let storage = create_storage(
        peripherals.spi2,
        peripherals.pins.gpio3,
        peripherals.pins.gpio0,
        peripherals.pins.gpio1,
        PinDriver::output(peripherals.pins.gpio2).unwrap(),
    );

    let ender = Arc::new(Mutex::new(Ender {
        serial,
        storage,
        driver,
    }));

    // let ender = setup(&mut peripherals);
    let mut server = create_server(&mut peripherals.modem);

    print_file_handler(&ender, &mut server);
    write_file_handler(&ender, &mut server);
    std::mem::forget(server);

    loop {
        FreeRtos::delay_ms(1000);
    }
}

struct Ender<B: BlockDev> {
    serial: SerialWrapper<'static>,
    storage: StorageWrapper<B>,
    driver: TWDTDriver<'static>,
}

fn print_file_handler<B: BlockDev>(ender: &Arc<Mutex<Ender<B>>>, server: &mut EspHttpServer) {
    let ender1 = ender.clone();
    server
        .fn_handler("/file/print", Method::Post, move |_| {
            let ender1 = ender1.clone();

            let builder = thread::Builder::new();
            builder
                .stack_size(10000)
                .spawn(move || {
                    match print_file(&ender1) {
                        Ok(_) => info!("File printed"),
                        Err(err) => error!("{err}"),
                    };
                })
                .unwrap();
            Ok(())
        })
        .unwrap();
}

fn print_file<B: BlockDev>(ender1: &Arc<Mutex<Ender<B>>>) -> Result<(), String> {
    let mut ender = ender1.lock().unwrap();
    let ender2 = ender.deref_mut();
    let mut watchdog = ender2.driver.watch_current_task().unwrap();
    ender2.serial.clear().map_err(|err| format!("{err:?}"))?;

    let mut reader = ender2.storage.get_reader();

    while let Some(line) = reader.read().unwrap() {
        // info!("Line from SD card: {}", line);
        ender2.serial.write(line, &mut watchdog).unwrap();

        let total_read_of_file =
            (reader.file_size_in_bytes() - reader.remaining_bytes_in_file()) as f32;
        info!(
            "{}%",
            100f32 * (total_read_of_file / reader.file_size_in_bytes() as f32),
        );
    }

    Ok(())
}

fn write_file_handler<B: BlockDev>(ender: &Arc<Mutex<Ender<B>>>, server: &mut EspHttpServer) {
    let ender1 = ender.clone();
    server
        .fn_handler("/file/write", Method::Post, move |mut request| {
            let mut ender = ender1.lock().unwrap();
            let ender2 = ender.deref_mut();
            let mut watchdog = ender2.driver.watch_current_task().unwrap();

            let Some(content_length) = request.content_len() else {
                return Err("No content length".into());
            };

            info!("Content length: {}", content_length);

            ender2.storage.delete().unwrap();

            let mut writer = ender2.storage.get_writer();

            let buffer = &mut [0u8; 1000];

            let mut total_read = 0f32;
            let mut last_instant = time::Instant::now();
            loop {
                let num_read = request.read(buffer).unwrap();

                if num_read == 0 {
                    break;
                }

                let s = core::str::from_utf8(&buffer[..num_read]).unwrap();
                writer.write(s).unwrap();

                total_read += num_read as f32;
                let time_diff = time::Instant::now().duration_since(last_instant);
                info!(
                    "{}%, {} bytes/second",
                    100f32 * (total_read / (content_length as f32)),
                    (num_read as f32) / (time_diff.as_secs_f32())
                );
                last_instant = time::Instant::now();
                watchdog.feed().unwrap();
                FreeRtos::delay_ms(10);
            }

            Ok(())
        })
        .unwrap();
}
