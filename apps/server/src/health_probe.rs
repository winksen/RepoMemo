//! `repomemo-server --health`: asks the server running on this machine whether it is ready and exits
//! with 0 or 1. It is the container's health check, so the image needs no `curl`.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::config::env_text;

const TIMEOUT: Duration = Duration::from_secs(4);

/// The check a container runs: 0 when `/health/ready` answers 200 on the configured address, else 1.
pub fn health_probe() -> i32 {
    let address = env_text("REPOMEMO_SERVER_ADDR").unwrap_or_else(|| "127.0.0.1:3020".to_owned());
    let Some(target) = local_target(&address) else {
        eprintln!("REPOMEMO_SERVER_ADDR ({address}) is not an address the health check can ask");
        return 1;
    };
    match status_of(target, "/health/ready") {
        Ok(200) => 0,
        Ok(status) => {
            eprintln!("The server answered {status} to /health/ready");
            1
        }
        Err(error) => {
            eprintln!("The server did not answer on {target}: {error}");
            1
        }
    }
}

/// Where this machine reaches a server that listens on `address`: an unspecified address (all
/// interfaces) is asked on the loopback.
fn local_target(address: &str) -> Option<SocketAddr> {
    let mut resolved = address.to_socket_addrs().ok()?;
    let mut target = resolved.next()?;
    if target.ip().is_unspecified() {
        target.set_ip(if target.is_ipv4() {
            std::net::Ipv4Addr::LOCALHOST.into()
        } else {
            std::net::Ipv6Addr::LOCALHOST.into()
        });
    }
    Some(target)
}

/// The status code of a plain `GET` of `path`, by hand, since a health check needs no HTTP client.
fn status_of(target: SocketAddr, path: &str) -> std::io::Result<u16> {
    let mut stream = TcpStream::connect_timeout(&target, TIMEOUT)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )?;
    let mut head = [0_u8; 32];
    let mut read = 0;
    while read < head.len() {
        let count = stream.read(&mut head[read..])?;
        if count == 0 {
            break;
        }
        read += count;
        if head[..read].contains(&b'\n') {
            break;
        }
    }
    String::from_utf8_lossy(&head[..read])
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "not an HTTP answer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn answering(reply: &'static str) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0_u8; 256];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        address
    }

    #[test]
    fn a_ready_server_is_200_and_an_unready_one_is_503() {
        let ready = answering("HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok");
        assert_eq!(status_of(ready, "/health/ready").unwrap(), 200);
        let unready = answering("HTTP/1.1 503 Service Unavailable\r\n\r\n");
        assert_eq!(status_of(unready, "/health/ready").unwrap(), 503);
    }

    #[test]
    fn nothing_listening_and_a_non_http_answer_are_errors() {
        let free = TcpListener::bind("127.0.0.1:0").unwrap();
        let closed = free.local_addr().unwrap();
        drop(free);
        assert!(status_of(closed, "/health/ready").is_err());
        let garbage = answering("hello\r\n");
        assert!(status_of(garbage, "/health/ready").is_err());
    }

    #[test]
    fn an_address_for_all_interfaces_is_asked_on_the_loopback() {
        assert_eq!(local_target("0.0.0.0:3020").unwrap().to_string(), "127.0.0.1:3020");
        assert_eq!(local_target("[::]:3020").unwrap().to_string(), "[::1]:3020");
        assert_eq!(local_target("127.0.0.1:4000").unwrap().to_string(), "127.0.0.1:4000");
    }
}
