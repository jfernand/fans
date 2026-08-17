use std::fs::File;
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
    // ZeroMQ PUSH socket connects to the receiver's PULL socket
    let ctx = zmq::Context::new();
    let socket = ctx.socket(zmq::PUSH).expect("failed to create ZMQ PUSH socket");
    let endpoint = "tcp://127.0.0.1:1337";
    socket.connect(endpoint).expect("failed to connect ZMQ PUSH to receiver");

    loop {
        match read_ec_io() {
            Ok(buf) => {
                if let Err(e) = socket.send(buf, 0) {
                    eprintln!("ZMQ send error: {}", e);
                } else {
                    print!(".");
                }
            }
            Err(e) => eprintln!("read_ec_io error: {}", e),
        }
        thread::sleep(Duration::from_millis(100));
    }
}