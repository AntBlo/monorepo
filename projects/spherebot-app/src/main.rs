use bit_field::BitField;
use std::error::Error;
use std::process::exit;
use std::{
    io::Write,
    net::{Shutdown, TcpStream},
};
use winit::event::RawKeyEvent;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut stream = TcpStream::connect("192.168.1.200:80").unwrap();
    stream.set_write_timeout(None).unwrap();
    stream.set_read_timeout(None).unwrap();

    let center = 0usize;
    let lean = 1usize;

    let stop = 2usize;
    let direction = 3usize;

    let mut pressed_a = false;
    let mut pressed_d = false;
    let mut pressed_w = false;
    let mut pressed_s = false;

    let event_loop = EventLoop::new().unwrap();

    let mut command_byte = 0u8;

    event_loop
        .run(|event, _| match event {
            Event::WindowEvent { event, .. } => {
                if event == WindowEvent::CloseRequested {
                    println!("Window closed, exiting program");
                }
            }
            Event::DeviceEvent {
                event:
                    winit::event::DeviceEvent::Key(RawKeyEvent {
                        state,
                        physical_key: PhysicalKey::Code(key),
                    }),
                ..
            } => {
                let is_pressed = state.is_pressed();
                match key {
                    KeyCode::KeyA => {
                        pressed_a = is_pressed;
                    }
                    KeyCode::KeyD => {
                        pressed_d = is_pressed;
                    }
                    KeyCode::KeyW => {
                        pressed_w = is_pressed;
                    }
                    KeyCode::KeyS => {
                        pressed_s = is_pressed;
                    }
                    KeyCode::Escape => {
                        println!("Esc key pressed, exiting program");
                        exit(0);
                    }
                    _ => {
                        return;
                    }
                }

                match (pressed_a, pressed_d) {
                    (true, true) => {}
                    (true, false) => {
                        command_byte.set_bit(center, false);
                        command_byte.set_bit(lean, true);
                    }
                    (false, true) => {
                        command_byte.set_bit(center, false);
                        command_byte.set_bit(lean, false);
                    }
                    (false, false) => {
                        command_byte.set_bit(center, true);
                    }
                }

                match (pressed_w, pressed_s) {
                    (true, true) => {}
                    (true, false) => {
                        command_byte.set_bit(stop, false);
                        command_byte.set_bit(direction, true);
                    }
                    (false, true) => {
                        command_byte.set_bit(stop, false);
                        command_byte.set_bit(direction, false);
                    }
                    (false, false) => {
                        command_byte.set_bit(stop, true);
                    }
                }
                stream.write_all(&[command_byte]).unwrap();
                stream.flush().unwrap();
            }
            _ => {}
        })
        .unwrap();

    let mut command_byte = 0u8;
    command_byte.set_bit(center, true);
    command_byte.set_bit(stop, true);
    stream.write_all(&[command_byte]).unwrap();
    stream.flush().unwrap();
    stream.shutdown(Shutdown::Both).unwrap();

    Ok(())
}
