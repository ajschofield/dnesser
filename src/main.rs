use std::net::UdpSocket;
use log::{info, debug, error, warn, LevelFilter};
use simplelog::{CombinedLogger, Config, TermLogger, TerminalMode, ColorChoice};

// Fixes "Header doesn't implement std::fmt::Debug [E0277]"
#[derive(Debug)]
struct Header {
    id: u16,
    flags: u16,
    qdcount: u16,
    ancount: u16,
    nscount: u16,
    arcount: u16,
}

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
        match socket.recv_from(&mut buf) {
            Ok((size, src)) => {
                debug!("Received {} bytes from {}", size, src);
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

                let header_raw = &buf[..12];
                let header = Header {
                    id: u16::from_be_bytes([header_raw[0], header_raw[1]]),
                    flags: u16::from_be_bytes([header_raw[2], header_raw[3]]),
                    qdcount: u16::from_be_bytes([header_raw[4], header_raw[5]]),
                    ancount: u16::from_be_bytes([header_raw[6], header_raw[7]]),
                    nscount: u16::from_be_bytes([header_raw[8], header_raw[9]]),
                    arcount: u16::from_be_bytes([header_raw[10], header_raw[11]]),
                };

                // Log parsed header
                debug!("Parsed header: {:?}", header);

                // First, create a new String variable to store the domain name
                let mut qname = String::new();

                // We start at offset 12, which is right after the header. We have to
                // do this since the domain name can be variable.
                let mut cursor = 12;
                // Let's loop through the bytes to try to find the full domain name
                loop {
                    // We shouldn't read past the buffer size, so check for that.
                    if cursor >= size {
                        break;
                    }
                    // Read the length byte of the current label
                    let len = buf[cursor] as usize;
                    cursor += 1; // Increment by one, move past the length byte
                    // If the length is 0, we've hit the end of the name
                    if len == 0 {
                        break;
                    }
                    // We need to check if the label is fully inside the buffer, and
                    // if so we need to reject it (malformed)
                    if cursor + len > size {
                        warn!("QNAME: label length exceeds buffer size");
                        break;
                    }
                    // We now know how to slice the buf to get the current label in
                    // the current iteration of the sequence
                    let label_raw = &buf[cursor..cursor + len];
                    let label = String::from_utf8(label_raw.to_vec()).unwrap();

                    // Push it into the variable so that as each iteration completes
                    // the qname can be constructed
                    qname.push_str(&label);
                    // On the final iteration of getting the last label, which will
                    // be the TLD, because there are no checks whether the loop is
                    // nearing completion an additional '.' will be added.
                    // It's a side effect of the logic, but apparently is technically
                    // correct in DNS.
                    qname.push('.');

                    // And we go around again!
                    cursor += len;
                }

                debug!("Gotcha! QNAME: {:?}", qname);

                // Since QTYPE and QCLASS are both 2 bytes, we can read them directly
                if cursor + 4 > size {
                    warn!("QTYPE/QCLASS: packet too short - cannot read");
                    continue;
                }

                // QTYPE is 2 bytes so we read from cursor to cursor + 2
                let qtype_raw = &buf[cursor..cursor + 2];
                let qtype = u16::from_be_bytes([qtype_raw[0], qtype_raw[1]]);
                debug!("Gotcha! QTYPE: {}", qtype);

                // Again, QCLASS is 2 bytes so we read from cursor + 2 to cursor + 4
                let qclass_raw = &buf[cursor + 2..cursor + 4];
                let qclass = u16::from_be_bytes([qclass_raw[0], qclass_raw[1]]);
                debug!("Gotcha! QCLASS: {}", qclass);

                // We can ignore the rest of the packet for now, as we have all the
                // information we need to process a basic query.
            }

            Err(e) => {
                error!("Failed to receive data: {}", e);
                continue;
            }
        }
    }
    }
