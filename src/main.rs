use std::net::UdpSocket;
use log::{info, debug, error, warn, LevelFilter};
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

    // First, bind to 0.0.0.0:53 (UDP) - this previously was 53535 but 
    // surprisingly macOS lets me bind on port 53 instead so we'll just do 
    // that I guess? This will be made more robust as development progresses.
    let socket = UdpSocket::bind("0.0.0.0:53")?;
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
        // address (IP & port).
        // Safe: recv_from won't write more than buf.len()
        let (size, src) = match socket.recv_from(&mut buf) {
            Ok(tuple) => tuple,
            Err(e) => {
                error!("recv_from issue: {}", e);
                continue;
            }
        };
        // Reject any packets that are larger than 512 bytes
        if size > 512 {
            warn!("Packet from {src} larger than 512 bytes (was {size}. Not a \
             valid query?");
            continue;
        }
        // Reject any packets that are shorter than 12 bytes, possibly
        // corrupted or truncated for whatever reason (although unlikely)
        if size < 12 {
            debug!("Packet from {src} too short (was {size}. Corruption?");
            continue;
        }
        // If the checks are happy, continue as normal
        debug!("Gotcha! Received {} bytes from {}", size, src);

        // For testing with dig command, trim buf to length of what client
        // sent and return to sender
        let response = &buf[..size];
        socket.send_to(response, &src)?;
    }
}