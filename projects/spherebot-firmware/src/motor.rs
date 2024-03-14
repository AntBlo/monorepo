use core::convert::Infallible;
use embassy_time::{Duration, Timer};
use embedded_hal::digital::OutputPin;
use esp_println::println;
use log::info;

type MotorPin = &'static mut dyn OutputPin<Error = Infallible>;

pub struct Motor {
    index: usize,
    pins: [MotorPin; 4],
}

impl Motor {
    pub fn new(pin0: MotorPin, pin1: MotorPin, pin2: MotorPin, pin3: MotorPin) -> Self {
        pin0.set_low().unwrap();
        pin1.set_low().unwrap();
        pin2.set_low().unwrap();
        pin3.set_low().unwrap();
        Self {
            index: 0,
            pins: [pin0, pin1, pin2, pin3],
        }
    }

    pub async fn stop(&mut self) {
        for pin in &mut self.pins {
            pin.set_low().unwrap();
        }
        Timer::after(Duration::from_millis(2)).await;
    }

    pub async fn forward(&mut self) {
        self.pins.get_mut(self.index).unwrap().set_low().unwrap();
        self.index = (self.index + 1) % self.pins.len();
        self.pins.get_mut(self.index).unwrap().set_high().unwrap();
        Timer::after(Duration::from_millis(2)).await;
    }

    pub async fn backward(&mut self) {
        self.pins.get_mut(self.index).unwrap().set_low().unwrap();
        let last_position = self.pins.len() - 1;
        self.index = self.index.checked_sub(1).unwrap_or(last_position);
        self.pins.get_mut(self.index).unwrap().set_high().unwrap();
        Timer::after(Duration::from_millis(2)).await;
    }
}

impl Drop for Motor {
    fn drop(&mut self) {
        for pin in &mut self.pins {
            pin.set_low().ok();
        }
    }
}
