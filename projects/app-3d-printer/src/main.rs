#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{fs::read_to_string, time::Duration};

fn main() {
    env_logger::init();

    let file = read_to_string("CE3_sphere_bot-weight_arm.gcode").unwrap();
    // dbg!(file.as_bytes().len());

    let client = reqwest::blocking::Client::new();

    let res = client
        .post("http://192.168.1.103/file/write")
        .timeout(Duration::from_secs(1000000))
        .body(file)
        .send()
        .unwrap();

    // dbg!(res);

    let res = client
        .post("http://192.168.1.103/file/print")
        .timeout(Duration::from_secs(1000000))
        .send()
        .unwrap();

    dbg!(res);

    // let options = eframe::NativeOptions {
    //     initial_window_size: Some(egui::vec2(320.0, 240.0)),
    //     ..Default::default()
    // };

    // eframe::run_simple_native("My egui App", options, move |ctx, _frame| {
    //     ctx.input(|i| {
    //         let a = &i.raw;
    //         dbg!(a);
    //     });
    //     egui::CentralPanel::default().show(ctx, |ui| {});
    // })
    // .unwrap();
}
