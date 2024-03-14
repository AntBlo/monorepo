use socketcan::{BlockingCan, CanFrame, CanSocket, Frame, Socket};

fn main() {
    // sudo ip link set can0 up type can bitrate 500000
    let mut sock = CanSocket::open("can0").unwrap();
    loop {
        wait_for_vin_request(&mut sock);

        let data = &[0x10, 0x14, 0x49, 0x02, 0x01, 72, 69, 76];
        write_frame(data, &sock);

        wait_for_flow_control(&mut sock);

        let data = &[0x21, 76, 79, 32, 87, 79, 82, 76];
        write_frame(data, &sock);

        wait_for_flow_control(&mut sock);

        let data = &[0x22, 68, 0, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa];
        write_frame(data, &sock);

        println!("end loop");
    }
}

fn write_frame(data: &[u8; 8], sock: &CanSocket) {
    let frame = &CanFrame::from_raw_id(0x7E8, data).unwrap();
    println!("write: {}", &frame_to_string(frame));
    sock.write_frame(frame).unwrap();
}

fn wait_for_vin_request(sock: &mut CanSocket) {
    loop {
        let frame = sock.receive().unwrap();
        let id = format!("{:X}", frame.raw_id());
        println!("read: {}", frame_to_string(&frame));
        if id == "7DF" {
            break;
        }
    }
}

fn wait_for_flow_control(sock: &mut CanSocket) {
    loop {
        let frame = sock.receive().unwrap();
        let id = format!("{:X}", frame.raw_id());
        println!("read: {}", frame_to_string(&frame));
        if id == "7E0" {
            break;
        }
    }
}

fn frame_to_string<F: Frame>(frame: &F) -> String {
    let id = frame.raw_id();
    let data_string = frame
        .data()
        .iter()
        .fold(String::from(""), |a, b| format!("{} {:02x}", a, b));

    format!("{:X}  [{}] {}", id, frame.dlc(), data_string)
}
