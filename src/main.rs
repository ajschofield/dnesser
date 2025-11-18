use std::net::UdpSocket;
use log::{info, debug, LevelFilter};
use simplelog::{CombinedLogger, Config, TermLogger, TerminalMode, ColorChoice};

fn main() -> std::io::Result<()> {
    let log_level = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    CombinedLogger::init(
        vec![
            TermLogger::new(log_level, Config::default(),
                            TerminalMode::Mixed, ColorChoice::Auto),
        ]
    ).unwrap();

    // First, bind to 0.0.0.0:53535 (UDP)
    // CHANGEME Change to 0.0.0.0 and port 53 when further in development
    let socket = UdpSocket::bind("0.0.0.0:53535")?;
    info!("Listening on {}", socket.local_addr()?);

    loop {
        // Create a zero-filled 512-byte buffer on every iteration - this is
        // completely synchronous and can only handle one query at a time in
        // its current state. If two clients send a query at the same time,
        // then the kernel will queue the second one until the first is handled.
        // TODO Add async functionality further in development
        // The 512 byte limit follows RFC 1035, and will do until EDNS is
        // supported.
        let mut buf = [0u8; 512];
        // This will block until a UDP packet arrives, and on success returns
        // a tuple: size -> number of bytes client sent; src -> client socket
        // address (IP & port)
        let (size, src) = socket.recv_from(&mut buf)?;
        debug!("Received {} bytes from {}", size, src);
        // For testing with dig command, trim buf to length of what client
        // sent and return to sender
        let response = &buf[..size];
        socket.send_to(response, &src)?;
    }
}