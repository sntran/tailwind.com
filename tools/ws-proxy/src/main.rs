//! A bridge between standard input and output and a WebSocket.
//!
//! Usage: ws-proxy wss://HOST/
//!
//! It is the `ProxyCommand` of `ssh` for the workflow "Remote SSH"
//! (`scripts/remote_ssh_client.sh`): the SSH server of a runner is behind
//! a tunnel of Cloudflare, which carries the bytes of the TCP connection
//! in binary WebSocket messages. `cloudflared access tcp` does the same
//! work; this program needs no download of cloudflared.
//!
//! The connection uses the proxy of `HTTPS_PROXY` when it is set, and the
//! certificates of `SSL_CERT_FILE`, else the roots of the web.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, TryRecvError};
use std::sync::Arc;
use std::time::Duration;
use tungstenite::{Message, WebSocket};

fn fail(text: impl std::fmt::Display) -> ! {
    eprintln!("ws-proxy: {text}");
    std::process::exit(1);
}

/// The host and the port of a URL such as `http://host:port/`.
fn host_port(url: &str, default_port: u16) -> (String, u16) {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let authority = rest.split('/').next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    match authority.rsplit_once(':') {
        Some((host, port)) => (host.to_string(), port.parse().unwrap_or(default_port)),
        None => (authority.to_string(), default_port),
    }
}

/// A TCP connection to the host, through the proxy of the environment.
fn connect(host: &str, port: u16) -> TcpStream {
    let proxy = ["HTTPS_PROXY", "https_proxy"].iter().find_map(|name| std::env::var(name).ok()).filter(|p| !p.is_empty());
    let Some(proxy) = proxy else {
        return TcpStream::connect((host, port)).unwrap_or_else(|e| fail(format!("{host}:{port}: {e}")));
    };
    let (proxy_host, proxy_port) = host_port(&proxy, 8080);
    let mut stream = TcpStream::connect((proxy_host.as_str(), proxy_port))
        .unwrap_or_else(|e| fail(format!("the proxy {proxy_host}:{proxy_port}: {e}")));
    write!(stream, "CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n\r\n").unwrap_or_else(|e| fail(e));
    // The answer of the proxy ends at an empty line.
    let mut answer = Vec::new();
    let mut byte = [0u8; 1];
    while !answer.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => answer.push(byte[0]),
            _ => fail("the proxy closed the connection"),
        }
    }
    let status = String::from_utf8_lossy(&answer);
    if !status.split(' ').nth(1).is_some_and(|code| code == "200") {
        fail(format!("the proxy refused the connection: {}", status.lines().next().unwrap_or("")));
    }
    stream
}

fn tls_config() -> rustls::ClientConfig {
    let mut roots = rustls::RootCertStore::empty();
    match std::env::var("SSL_CERT_FILE") {
        Ok(file) if !file.is_empty() => {
            let data = std::fs::read(&file).unwrap_or_else(|e| fail(format!("{file}: {e}")));
            for cert in rustls_pemfile::certs(&mut data.as_slice()).flatten() {
                let _ = roots.add(cert);
            }
        }
        _ => roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned()),
    }
    rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth()
}

fn main() {
    let url = std::env::args().nth(1).unwrap_or_else(|| fail("usage: ws-proxy wss://HOST/"));
    if !url.starts_with("wss://") {
        fail("the URL must start with wss://");
    }
    let (host, port) = host_port(&url, 443);

    let tcp = connect(&host, port);
    let name = rustls::pki_types::ServerName::try_from(host.clone()).unwrap_or_else(|e| fail(e));
    let session = rustls::ClientConnection::new(Arc::new(tls_config()), name).unwrap_or_else(|e| fail(e));
    let tls = rustls::StreamOwned::new(session, tcp);
    let (mut socket, _): (WebSocket<_>, _) =
        tungstenite::client(url.as_str(), tls).unwrap_or_else(|e| fail(format!("{url}: {e}")));

    // After the handshake, the socket does not block: one loop moves the
    // bytes in the two directions.
    socket.get_ref().sock.set_nonblocking(true).unwrap_or_else(|e| fail(e));

    let (sender, input) = channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut buffer = [0u8; 32 * 1024];
        loop {
            match stdin.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if sender.send(buffer[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let mut stdout = std::io::stdout().lock();
    let would_block = |e: &tungstenite::Error| {
        matches!(e, tungstenite::Error::Io(io) if io.kind() == std::io::ErrorKind::WouldBlock)
    };
    let mut input_open = true;
    loop {
        let mut idle = true;

        // From the WebSocket to the output.
        match socket.read() {
            Ok(Message::Binary(data)) => {
                idle = false;
                if stdout.write_all(&data).and_then(|_| stdout.flush()).is_err() {
                    break;
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => idle = false,
            Err(e) if would_block(&e) => {}
            Err(_) => break,
        }

        // From the input to the WebSocket.
        if input_open {
            match input.try_recv() {
                Ok(data) => {
                    idle = false;
                    let mut result = socket.write(Message::Binary(data));
                    while matches!(&result, Err(e) if would_block(e)) {
                        std::thread::sleep(Duration::from_millis(1));
                        result = socket.flush();
                    }
                    if result.is_err() {
                        break;
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    // The end of the input: ssh is done.
                    input_open = false;
                    let _ = socket.close(None);
                }
            }
        }
        match socket.flush() {
            Ok(()) => {}
            Err(e) if would_block(&e) => {}
            Err(_) => break,
        }

        if idle {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
