use std::fs::File;
use std::net::UdpSocket;
use std::os::unix::fs::FileExt;
use std::thread;
use std::time::Duration;

fn read_ec_io() -> std::io::Result<Vec<u8>> {
    let f = File::open("/sys/kernel/debug/ec/ec0/io")?;
    let mut buf = vec![0u8; 256];
    f.read_at(&mut buf, 0)?;
    Ok(buf)
}

fn main() {
    // Create a UDP socket once; bind to an ephemeral local port.
    let socket = UdpSocket::bind("0.0.0.0:0").expect("failed to bind UDP socket");
    let dest = "127.0.0.1:1337";

    loop {
        match read_ec_io() {
            Ok(buf) => {
                match socket.send_to(&buf, dest) {
                    Ok(sent) => {
                        print!(".");
                        if sent != buf.len() {
                            eprintln!("warning: only sent {} of {} bytes", sent, buf.len());
                        }
                    }
                    Err(e) => eprintln!("UDP send_to error: {}", e),
                }
            }
            Err(e) => eprintln!("read_ec_io error: {}", e),
        }
        thread::sleep(Duration::from_millis(100));
    }
}