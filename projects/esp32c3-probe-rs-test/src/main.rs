#![cfg_attr(not(test), no_std)]
#![no_main]

extern crate alloc;
use core::mem::MaybeUninit;
use esp32c3_hal::prelude::*;
use esp_alloc::EspHeap;
// use esp_backtrace as _;
// use esp_println::println;
// use esp_wifi::{initialize, EspWifiInitFor};

use defmt_rtt as _;
use esp_backtrace as _;
use esp_println::println;
// use rtt_target::{rprintln, rtt_init_print};

#[global_allocator]
static ALLOCATOR: EspHeap = EspHeap::empty();

// fn init_heap() {
//     const HEAP_SIZE: usize = 32 * 1024;
//     static mut HEAP: MaybeUninit<[u8; HEAP_SIZE]> = MaybeUninit::uninit();

//     unsafe {
//         ALLOCATOR.init(HEAP.as_mut_ptr() as *mut u8, HEAP_SIZE);
//     }
// }

#[entry]
fn main() -> ! {
    let mut a = 0;
    loop {
        println!("for-loop iteration #{a}");
        a += 1;
    }
}
