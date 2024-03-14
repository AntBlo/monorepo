use std::time::{Duration, Instant};

use embedded_hal::{serial::Write, watchdog::Watchdog};
use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{AnyIOPin, InputPin, OutputPin},
    peripheral::Peripheral,
    uart::{config, Uart, UartDriver},
};
use log::{error, info};

pub fn create_serial<'a, UART: Uart>(
    uart: impl Peripheral<P = UART> + 'static,
    tx: impl Peripheral<P = impl OutputPin> + 'static,
    rx: impl Peripheral<P = impl InputPin> + 'static,
    config: &config::Config,
) -> SerialWrapper<'a> {
    let uart = esp_idf_hal::uart::UartDriver::new(
        uart,
        tx,
        rx,
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
        config,
    )
    .expect("Should create uart");

    SerialWrapper {
        uart,
        read_line_buffer: String::new(),
    }
}

pub struct SerialWrapper<'a> {
    uart: UartDriver<'a>,
    read_line_buffer: String,
}

impl<'a> SerialWrapper<'a> {
    pub fn write(
        &mut self,
        line: impl AsRef<str>,
        watchdog: &mut impl Watchdog,
    ) -> Result<(), SerialLineError> {
        if line.as_ref().is_empty() || line.as_ref().trim().starts_with(';') {
            return Ok(());
        }
 
        let report_temperatures = "M114\n";
        self.inner_write(report_temperatures).unwrap();

        loop {
            let Some(line) = self.read().unwrap() else {
                continue;
            };

            if line == "echo:busy: processing\n" {
                self.feed_watch_dog(watchdog);
            }

            if line.contains("X:")
                && line.contains("Y:")
                && line.contains("Z:")
                && line.contains("E:")
            {
                let ok = self.read().unwrap().unwrap();
                debug_assert_eq!(ok, "ok\n");
                break;
            }
        }
        loop {
            self.inner_write(&line).unwrap();
            self.feed_watch_dog(watchdog);
            let response = self.read().unwrap();
            let is_unknown_command = response
                .as_ref()
                .is_some_and(|s| s.starts_with("echo:Unknown command:"));
            if !is_unknown_command {
                break;
            }
            info!("unknown command: {response:?}");
            let ok = self.read().unwrap().unwrap();
            debug_assert_eq!(ok, "ok\n");
        }
        Ok(())
    }

    fn feed_watch_dog(&self, watchdog: &mut impl Watchdog) {
        watchdog.feed();
        FreeRtos::delay_ms(10);
    }

    fn inner_write(&mut self, line: impl AsRef<str>) -> Result<(), SerialLineError> {
        let mut line = line.as_ref();
        while !line.is_empty() {
            let remaining_write = match self.uart.remaining_write() {
                Ok(value) => value,
                Err(err) => {
                    error!("{err}");
                    continue;
                }
            };

            let remaining_write = core::cmp::min(remaining_write, line.len());
            let write_chunk = &line[..remaining_write];

            let num_written = self
                .uart
                .write(write_chunk.as_bytes())
                .map_err(|err| {
                    error!("{err:#?}");
                    SerialLineError::Write
                })
                .unwrap_or(0);

            line = &line[num_written..];
        }

        loop {
            match self.uart.flush() {
                Ok(_) => break,
                Err(err) => {
                    match err {
                        nb::Error::Other(_) => {
                            error!("{err:#?}");
                            return Err(SerialLineError::Write);
                        }
                        nb::Error::WouldBlock => continue,
                    };
                }
            }
        }

        Ok(())
    }

    pub fn read(&mut self) -> Result<Option<String>, SerialLineError> {
        let index_after_newline = match self.read_line_buffer.find('\n') {
            Some(newline_index) => newline_index + 1,
            None => {
                let mut buffer = [0u8; 1000];

                loop {
                    let remaining_read = self.uart.remaining_read().map_err(|err| {
                        error!("{err:#?}");
                        SerialLineError::Read
                    })?;

                    if remaining_read == 0 {
                        return Ok(None);
                    }

                    let remaining_read = core::cmp::min(remaining_read, buffer.len());

                    let num_read =
                        self.uart
                            .read(&mut buffer[..remaining_read], 10)
                            .map_err(|err| {
                                error!("{err:#?}");
                                SerialLineError::Read
                            })?;

                    self.read_line_buffer +=
                        core::str::from_utf8(&buffer[..num_read]).map_err(|err| {
                            error!("{err:#?}");
                            SerialLineError::Utf8Error
                        })?;

                    if let Some(newline_index) = self.read_line_buffer.find('\n') {
                        break newline_index + 1;
                    }
                }
            }
        };

        let (line, rest) = self.read_line_buffer.split_at(index_after_newline);
        let line = line.to_string();
        self.read_line_buffer = rest.to_string();

        Ok(Some(line))
    }

    pub fn clear(&mut self) -> Result<(), SerialLineError> {
        self.read_line_buffer.clear();
        self.uart.clear_rx().map_err(|err| {
            error!("{err}");
            SerialLineError::Clear
        })?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum SerialLineError {
    Write,
    Clear,
    Read,
    Utf8Error,
}
