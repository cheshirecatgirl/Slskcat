//! Outbound connections through an HTTP, `SOCKS4a` or SOCKS5 proxy, and the
//! local relay that puts the protocol library behind one.
//!
//! The library opens its own sockets and has no proxy support, so it is not
//! asked to. Instead the whole session is pointed at a loopback address this
//! module listens on:
//!
//! - the server connection is accepted here and re-dialled through the proxy;
//! - the peer addresses the server sends back are rewritten, in flight, to
//!   further loopback addresses, so every outbound peer connection the library
//!   makes also arrives here and is re-dialled through the proxy.
//!
//! Rewriting is possible without re-framing anything because the two fields
//! that matter are fixed width — four bytes of IPv4 and four of port — so a
//! patched message is exactly as long as the one it replaces.
//!
//! What this cannot do is accept connections. A peer that will not take a
//! direct connection is unreachable while a proxy is in use, and nothing can
//! be uploaded to one, because the listening socket is the one part of the
//! protocol a proxy has no answer for. Sessions using a proxy therefore run
//! with listening disabled rather than advertising a port nobody can reach.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// How long a proxy has to complete its handshake.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(20);

/// Distinct peers that may be mapped at once.
///
/// Each mapping costs a listening socket and a thread, so this is a ceiling on
/// what one session can spend rather than a limit anyone should reach: peer
/// addresses are requested when a transfer or a browse begins, not for every
/// search result.
const MAX_PEER_MAPPINGS: usize = 256;

/// Server message codes carrying a peer address.
const CODE_GET_PEER_ADDRESS: u32 = 3;
const CODE_CONNECT_TO_PEER: u32 = 18;

/// Which protocol a proxy speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProxyKind {
    /// `CONNECT` over HTTP. Named "HTTPS proxy" by most software that offers
    /// one, though the hop to the proxy itself is plain HTTP.
    Http,
    /// `SOCKS4a`. Plain SOCKS4 can only express an IPv4 destination, so the "a"
    /// form is always used — it is the one that lets the proxy resolve the
    /// hostname, which is what keeps the lookup off this machine.
    Socks4,
    /// SOCKS5, with username/password authentication when credentials are set.
    Socks5,
}

/// A proxy to make outbound connections through.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proxy {
    pub kind: ProxyKind,
    pub host: String,
    pub port: u16,
    /// Credentials, when the proxy asks for them. `SOCKS4a` carries only a user
    /// id and no password; the password is ignored there rather than sent
    /// somewhere it would not be understood.
    pub username: String,
    pub password: String,
}

impl Proxy {
    /// Whether this describes a usable proxy.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        !self.host.trim().is_empty() && self.port != 0
    }

    /// Open a connection to `host:port` through this proxy.
    ///
    /// The destination is always handed to the proxy as text. Resolving it
    /// here first would send the name to a local resolver and announce, to
    /// anyone watching that, exactly what the proxy was meant to conceal.
    ///
    /// # Errors
    /// If the proxy cannot be reached, rejects the credentials, or refuses the
    /// destination.
    pub fn connect(&self, host: &str, port: u16) -> io::Result<TcpStream> {
        let stream = TcpStream::connect((self.host.as_str(), self.port))?;
        stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
        stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;
        stream.set_nodelay(true)?;

        let mut stream = stream;
        match self.kind {
            ProxyKind::Http => self.shake_http(&mut stream, host, port)?,
            ProxyKind::Socks4 => self.shake_socks4(&mut stream, host, port)?,
            ProxyKind::Socks5 => self.shake_socks5(&mut stream, host, port)?,
        }

        // The handshake budget must not become the transfer budget: a download
        // sitting in a peer's queue is silent for as long as the peer likes.
        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        Ok(stream)
    }

    fn shake_http(&self, stream: &mut TcpStream, host: &str, port: u16) -> io::Result<()> {
        let mut request = format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n");
        if !self.username.is_empty() {
            use std::fmt::Write as _;
            let token = base64(format!("{}:{}", self.username, self.password).as_bytes());
            let _ = write!(request, "Proxy-Authorization: Basic {token}\r\n");
        }
        request.push_str("\r\n");
        stream.write_all(request.as_bytes())?;

        // Read only as far as the blank line: whatever follows is the tunnel
        // itself, and consuming any of it would lose the first bytes the
        // destination sent.
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            if head.len() > 8192 {
                return Err(refused("The proxy sent an oversized response."));
            }
            if stream.read(&mut byte)? == 0 {
                return Err(refused("The proxy closed the connection."));
            }
            head.push(byte[0]);
        }

        let status = String::from_utf8_lossy(&head);
        let ok = status
            .split_once(' ')
            .and_then(|(_, rest)| rest.split_whitespace().next())
            .is_some_and(|code| code.starts_with('2'));
        if ok {
            Ok(())
        } else {
            let line = status.lines().next().unwrap_or("no response").trim();
            Err(refused(&format!(
                "The proxy refused the connection: {line}"
            )))
        }
    }

    fn shake_socks4(&self, stream: &mut TcpStream, host: &str, port: u16) -> io::Result<()> {
        let mut request = vec![0x04, 0x01];
        request.extend_from_slice(&port.to_be_bytes());
        // 0.0.0.1 is the SOCKS4a marker for "the hostname follows"; the proxy
        // resolves it rather than this machine.
        request.extend_from_slice(&[0, 0, 0, 1]);
        request.extend_from_slice(self.username.as_bytes());
        request.push(0);
        request.extend_from_slice(host.as_bytes());
        request.push(0);
        stream.write_all(&request)?;

        let mut reply = [0u8; 8];
        stream.read_exact(&mut reply)?;
        match reply[1] {
            0x5a => Ok(()),
            0x5b => Err(refused("The proxy rejected the request.")),
            0x5c | 0x5d => Err(refused("The proxy could not verify the user id.")),
            other => Err(refused(&format!("The proxy replied with code {other}."))),
        }
    }

    fn shake_socks5(&self, stream: &mut TcpStream, host: &str, port: u16) -> io::Result<()> {
        // Offer only what is actually on the table. Advertising password auth
        // without credentials invites a challenge that cannot be answered.
        let methods: &[u8] = if self.username.is_empty() {
            &[0x00]
        } else {
            &[0x00, 0x02]
        };
        let mut greeting = vec![0x05, u8::try_from(methods.len()).unwrap_or(1)];
        greeting.extend_from_slice(methods);
        stream.write_all(&greeting)?;

        let mut choice = [0u8; 2];
        stream.read_exact(&mut choice)?;
        if choice[0] != 0x05 {
            return Err(refused("The proxy did not answer as SOCKS5."));
        }
        match choice[1] {
            0x00 => {}
            0x02 => self.authenticate_socks5(stream)?,
            0xff => return Err(refused("The proxy accepted none of the offered logins.")),
            other => return Err(refused(&format!("The proxy asked for login type {other}."))),
        }

        let name = host.as_bytes();
        let length = u8::try_from(name.len())
            .map_err(|_| refused("That destination name is too long for SOCKS5."))?;
        let mut request = vec![0x05, 0x01, 0x00, 0x03, length];
        request.extend_from_slice(name);
        request.extend_from_slice(&port.to_be_bytes());
        stream.write_all(&request)?;

        let mut head = [0u8; 4];
        stream.read_exact(&mut head)?;
        if head[1] != 0x00 {
            return Err(refused(socks5_reason(head[1])));
        }
        // The bound address comes back in a form whose length depends on its
        // type, and it has to be consumed before the tunnel begins.
        let mut discard = match head[3] {
            0x01 => vec![0u8; 4 + 2],
            0x04 => vec![0u8; 16 + 2],
            0x03 => {
                let mut length = [0u8; 1];
                stream.read_exact(&mut length)?;
                vec![0u8; usize::from(length[0]) + 2]
            }
            other => return Err(refused(&format!("The proxy sent address type {other}."))),
        };
        stream.read_exact(&mut discard)?;
        Ok(())
    }

    fn authenticate_socks5(&self, stream: &mut TcpStream) -> io::Result<()> {
        let user = self.username.as_bytes();
        let pass = self.password.as_bytes();
        let (Ok(user_len), Ok(pass_len)) = (u8::try_from(user.len()), u8::try_from(pass.len()))
        else {
            return Err(refused("Those proxy credentials are too long for SOCKS5."));
        };
        let mut request = vec![0x01, user_len];
        request.extend_from_slice(user);
        request.push(pass_len);
        request.extend_from_slice(pass);
        stream.write_all(&request)?;

        let mut reply = [0u8; 2];
        stream.read_exact(&mut reply)?;
        if reply[1] == 0 {
            Ok(())
        } else {
            Err(refused("The proxy rejected the credentials."))
        }
    }
}

fn refused(message: &str) -> io::Error {
    io::Error::other(message.to_owned())
}

fn socks5_reason(code: u8) -> &'static str {
    match code {
        0x02 => "The proxy is configured to disallow that connection.",
        0x03 => "The proxy reported the network as unreachable.",
        0x04 => "The proxy reported the host as unreachable.",
        0x05 => "The destination refused the connection.",
        0x06 => "The connection through the proxy timed out.",
        _ => "The proxy refused the connection.",
    }
}

/// Minimal base64, for the one header that needs it.
///
/// A dependency for sixteen lines would be a poor trade in a client whose
/// argument is that it carries almost nothing.
fn base64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                let index = usize::try_from((n >> (18 - i * 6)) & 0x3f).unwrap_or(0);
                out.push(char::from(ALPHABET[index]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Loopback addresses standing in for the server and for each peer.
///
/// Started before the session and stopped with it. Holding one is what makes
/// the library's own sockets go through the proxy without the library knowing
/// anything about it.
pub struct Relay {
    server_addr: SocketAddrV4,
    inner: Arc<Shared>,
}

struct Shared {
    proxy: Proxy,
    /// Real peer address for each loopback port standing in for one.
    peers: Mutex<HashMap<SocketAddrV4, u16>>,
    running: AtomicBool,
}

impl Relay {
    /// Bind the loopback stand-in for `host:port` and start serving it.
    ///
    /// # Errors
    /// If the loopback listener cannot be bound.
    pub fn start(proxy: Proxy, host: String, port: u16) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let SocketAddr::V4(server_addr) = listener.local_addr()? else {
            return Err(refused("Loopback did not return an IPv4 address."));
        };

        let inner = Arc::new(Shared {
            proxy,
            peers: Mutex::new(HashMap::new()),
            running: AtomicBool::new(true),
        });

        let shared = Arc::clone(&inner);
        thread::spawn(move || {
            for incoming in listener.incoming() {
                if !shared.running.load(Ordering::Relaxed) {
                    return;
                }
                let Ok(client) = incoming else { continue };
                let shared = Arc::clone(&shared);
                let host = host.clone();
                thread::spawn(move || {
                    if let Err(error) = shared.serve_server(client, &host, port) {
                        crate::proxy::log_relay_end(&error);
                    }
                });
            }
        });

        Ok(Self { server_addr, inner })
    }

    /// Where the library should be told the Soulseek server is.
    #[must_use]
    pub const fn server_addr(&self) -> SocketAddrV4 {
        self.server_addr
    }

    /// Stop accepting. Connections already open end with their sockets.
    pub fn stop(&self) {
        self.inner.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A relay ending is ordinary — a peer hung up, a transfer finished — so it is
/// not surfaced as a warning. This exists to name that decision rather than
/// leave a silently discarded error.
fn log_relay_end(_error: &io::Error) {}

impl Shared {
    /// Relay the server connection, rewriting peer addresses on the way in.
    fn serve_server(self: &Arc<Self>, client: TcpStream, host: &str, port: u16) -> io::Result<()> {
        let upstream = self.proxy.connect(host, port)?;
        client.set_nodelay(true)?;

        // Client to server needs no inspection: nothing we send carries an
        // address the library could be misled by.
        let outbound_client = client.try_clone()?;
        let outbound_upstream = upstream.try_clone()?;
        thread::spawn(move || {
            let _ = pipe(outbound_client, outbound_upstream);
        });

        self.rewrite_inbound(upstream, client)
    }

    /// Copy server to client one whole message at a time, replacing the peer
    /// addresses in the two messages that carry them.
    fn rewrite_inbound(self: &Arc<Self>, mut from: TcpStream, mut to: TcpStream) -> io::Result<()> {
        loop {
            let mut length_bytes = [0u8; 4];
            if let Err(error) = from.read_exact(&mut length_bytes) {
                return if error.kind() == io::ErrorKind::UnexpectedEof {
                    Ok(())
                } else {
                    Err(error)
                };
            }
            let length = u32::from_le_bytes(length_bytes) as usize;
            // A length the server would never send is either corruption or
            // someone else on the wire; either way it is not worth allocating
            // for.
            if !(4..=16 * 1024 * 1024).contains(&length) {
                return Err(refused("The server sent a message of implausible length."));
            }

            let mut body = vec![0u8; length];
            from.read_exact(&mut body)?;
            self.patch_message(&mut body);

            to.write_all(&length_bytes)?;
            to.write_all(&body)?;
            to.flush()?;
        }
    }

    /// Replace the address in `body` if it is a message that carries one.
    ///
    /// The patch is exactly as long as what it replaces — four bytes of IPv4
    /// and four of port — so the framing this was read with still holds.
    fn patch_message(self: &Arc<Self>, body: &mut [u8]) {
        let Some(code) = body
            .get(..4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        else {
            return;
        };
        // Both carry a username first; `ConnectToPeer` carries a type string
        // after it, and the address follows whatever strings there are.
        let strings = match code {
            CODE_GET_PEER_ADDRESS => 1,
            CODE_CONNECT_TO_PEER => 2,
            _ => return,
        };

        let mut at = 4;
        for _ in 0..strings {
            let Some(field) = body.get(at..at + 4) else {
                return;
            };
            let size = u32::from_le_bytes([field[0], field[1], field[2], field[3]]) as usize;
            at = match at.checked_add(4).and_then(|a| a.checked_add(size)) {
                Some(next) if next <= body.len() => next,
                _ => return,
            };
        }

        let Some(address) = body.get(at..at + 8) else {
            return;
        };
        // The address arrives with its octets reversed, which is how the
        // protocol carries them.
        // The port travels as a 32-bit field but only ever holds a port, so
        // anything above 16 bits is a message this is not equipped to rewrite.
        let wide = u32::from_le_bytes([address[4], address[5], address[6], address[7]]);
        let Ok(port) = u16::try_from(wide) else {
            return;
        };
        let real = SocketAddrV4::new(
            Ipv4Addr::new(address[3], address[2], address[1], address[0]),
            port,
        );
        if real.ip().is_unspecified() || real.port() == 0 {
            return;
        }
        let Some(local) = self.map_peer(real) else {
            return;
        };

        body[at] = 1;
        body[at + 1] = 0;
        body[at + 2] = 0;
        body[at + 3] = 127;
        body[at + 4..at + 8].copy_from_slice(&u32::from(local).to_le_bytes());
    }

    /// The loopback port standing in for `real`, binding one if needed.
    fn map_peer(self: &Arc<Self>, real: SocketAddrV4) -> Option<u16> {
        let mut peers = self.peers.lock().ok()?;
        if let Some(port) = peers.get(&real) {
            return Some(*port);
        }
        if peers.len() >= MAX_PEER_MAPPINGS {
            return None;
        }

        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
        let SocketAddr::V4(local) = listener.local_addr().ok()? else {
            return None;
        };
        peers.insert(real, local.port());
        drop(peers);

        let shared = Arc::clone(self);
        thread::spawn(move || {
            for incoming in listener.incoming() {
                if !shared.running.load(Ordering::Relaxed) {
                    return;
                }
                let Ok(client) = incoming else { continue };
                let shared = Arc::clone(&shared);
                thread::spawn(move || {
                    let _ = shared.serve_peer(client, real);
                });
            }
        });
        Some(local.port())
    }

    /// Relay one peer connection through the proxy, untouched in both
    /// directions — a peer stream carries no addresses to rewrite.
    fn serve_peer(self: &Arc<Self>, client: TcpStream, real: SocketAddrV4) -> io::Result<()> {
        let upstream = self.proxy.connect(&real.ip().to_string(), real.port())?;
        client.set_nodelay(true)?;
        upstream.set_nodelay(true)?;

        let up_from = client.try_clone()?;
        let up_to = upstream.try_clone()?;
        thread::spawn(move || {
            let _ = pipe(up_from, up_to);
        });
        pipe(upstream, client)
    }
}

/// Copy until one side ends, then close the other so its reader wakes.
fn pipe(mut from: TcpStream, mut to: TcpStream) -> io::Result<()> {
    let result = io::copy(&mut from, &mut to).map(|_| ());
    let _ = to.shutdown(std::net::Shutdown::Write);
    let _ = from.shutdown(std::net::Shutdown::Read);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;

    fn proxy_at(kind: ProxyKind, addr: SocketAddrV4) -> Proxy {
        Proxy {
            kind,
            host: addr.ip().to_string(),
            port: addr.port(),
            username: String::new(),
            password: String::new(),
        }
    }

    /// A listener that runs `serve` once against the first connection, and the
    /// address to point a proxy at.
    fn fake_proxy(serve: impl FnOnce(TcpStream) + Send + 'static) -> SocketAddrV4 {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let SocketAddr::V4(addr) = listener.local_addr().unwrap() else {
            unreachable!("bound to loopback v4")
        };
        thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                serve(stream);
            }
        });
        addr
    }

    #[test]
    fn socks5_asks_for_the_name_rather_than_resolving_it() {
        // The whole point of the proxy is defeated if the destination is
        // looked up here first, so the request must carry the hostname.
        let seen = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&seen);
        let addr = fake_proxy(move |mut stream| {
            let mut greeting = [0u8; 3];
            stream.read_exact(&mut greeting).unwrap();
            stream.write_all(&[0x05, 0x00]).unwrap();

            let mut head = [0u8; 5];
            stream.read_exact(&mut head).unwrap();
            let mut name = vec![0u8; usize::from(head[4])];
            stream.read_exact(&mut name).unwrap();
            let mut port = [0u8; 2];
            stream.read_exact(&mut port).unwrap();
            *captured.lock().unwrap() = name.clone();

            // Success, bound to 0.0.0.0:0.
            stream
                .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .unwrap();
            assert_eq!(head[3], 0x03, "address type must be a domain name");
            assert_eq!(u16::from_be_bytes(port), 2416);
        });

        let stream = proxy_at(ProxyKind::Socks5, addr)
            .connect("server.slsknet.org", 2416)
            .expect("the handshake should complete");
        drop(stream);
        assert_eq!(
            String::from_utf8(seen.lock().unwrap().clone()).unwrap(),
            "server.slsknet.org"
        );
    }

    #[test]
    fn socks5_reports_a_refusal_rather_than_returning_a_dead_socket() {
        let addr = fake_proxy(|mut stream| {
            let mut greeting = [0u8; 3];
            stream.read_exact(&mut greeting).unwrap();
            stream.write_all(&[0x05, 0x00]).unwrap();
            let mut head = [0u8; 5];
            stream.read_exact(&mut head).unwrap();
            let mut rest = vec![0u8; usize::from(head[4]) + 2];
            stream.read_exact(&mut rest).unwrap();
            // 0x05: connection refused by destination.
            stream
                .write_all(&[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .unwrap();
        });

        let error = proxy_at(ProxyKind::Socks5, addr)
            .connect("example.invalid", 2416)
            .expect_err("a refusal must not look like success");
        assert!(error.to_string().contains("refused"));
    }

    #[test]
    fn socks4_uses_the_a_form_so_the_proxy_resolves_the_name() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&seen);
        let addr = fake_proxy(move |mut stream| {
            let mut head = [0u8; 8];
            stream.read_exact(&mut head).unwrap();
            let mut rest = Vec::new();
            let mut byte = [0u8; 1];
            // The user id and the hostname, each NUL-terminated.
            let mut nulls = 0;
            while nulls < 2 {
                stream.read_exact(&mut byte).unwrap();
                if byte[0] == 0 {
                    nulls += 1;
                } else {
                    rest.push(byte[0]);
                }
            }
            *captured.lock().unwrap() = rest;
            assert_eq!(&head[4..8], &[0, 0, 0, 1], "the SOCKS4a marker");
            stream.write_all(&[0x00, 0x5a, 0, 0, 0, 0, 0, 0]).unwrap();
        });

        proxy_at(ProxyKind::Socks4, addr)
            .connect("server.slsknet.org", 2416)
            .expect("the handshake should complete");
        assert_eq!(
            String::from_utf8(seen.lock().unwrap().clone()).unwrap(),
            "server.slsknet.org"
        );
    }

    #[test]
    fn http_connect_sends_the_tunnel_request_and_stops_at_the_blank_line() {
        let addr = fake_proxy(|stream| {
            let mut reader = io::BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                request.push_str(&line);
            }
            assert!(request.starts_with("CONNECT server.slsknet.org:2416 HTTP/1.1\r\n"));
            let mut stream = stream;
            // The byte after the blank line belongs to the tunnel; a reader
            // that swallowed it would lose the destination's first word.
            stream
                .write_all(b"HTTP/1.1 200 Connection established\r\n\r\nHELLO")
                .unwrap();
        });

        let mut stream = proxy_at(ProxyKind::Http, addr)
            .connect("server.slsknet.org", 2416)
            .expect("the handshake should complete");
        let mut first = [0u8; 5];
        stream.read_exact(&mut first).unwrap();
        assert_eq!(
            &first, b"HELLO",
            "the tunnel must start where the head ended"
        );
    }

    #[test]
    fn http_refusal_carries_the_status_line() {
        let addr = fake_proxy(|mut stream| {
            let mut sink = [0u8; 1];
            while stream.read(&mut sink).unwrap_or(0) > 0 {
                if sink[0] == b'\n' {
                    break;
                }
            }
            let _ = stream.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n");
        });

        let error = proxy_at(ProxyKind::Http, addr)
            .connect("server.slsknet.org", 2416)
            .expect_err("407 is not success");
        assert!(error.to_string().contains("407"), "{error}");
    }

    #[test]
    fn base64_matches_the_known_answers() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(b"user:pass"), "dXNlcjpwYXNz");
    }

    /// Build a server message: length, code, then the given body.
    fn framed(code: u32, body: &[u8]) -> Vec<u8> {
        let mut out = code.to_le_bytes().to_vec();
        out.extend_from_slice(body);
        out
    }

    fn string_field(text: &str) -> Vec<u8> {
        let mut out = u32::try_from(text.len()).unwrap().to_le_bytes().to_vec();
        out.extend_from_slice(text.as_bytes());
        out
    }

    fn shared_for_test() -> Arc<Shared> {
        Arc::new(Shared {
            proxy: Proxy {
                kind: ProxyKind::Socks5,
                host: "127.0.0.1".into(),
                port: 1080,
                username: String::new(),
                password: String::new(),
            },
            peers: Mutex::new(HashMap::new()),
            running: AtomicBool::new(true),
        })
    }

    #[test]
    fn a_peer_address_is_replaced_by_a_loopback_one_of_the_same_length() {
        let shared = shared_for_test();
        let mut body = framed(CODE_GET_PEER_ADDRESS, &string_field("velvet_hare"));
        // 203.0.113.9, octets reversed as the protocol carries them.
        body.extend_from_slice(&[9, 113, 0, 203]);
        body.extend_from_slice(&2234u32.to_le_bytes());
        let before = body.len();

        shared.patch_message(&mut body);

        assert_eq!(body.len(), before, "the framing must still describe it");
        let at = 4 + 4 + "velvet_hare".len();
        assert_eq!(&body[at..at + 4], &[1, 0, 0, 127], "rewritten to loopback");
        let port = u32::from_le_bytes([body[at + 4], body[at + 5], body[at + 6], body[at + 7]]);
        assert_ne!(port, 2234, "and to a port of ours, not the peer's");
        assert_ne!(port, 0);
    }

    #[test]
    fn the_same_peer_keeps_the_same_stand_in() {
        // Otherwise every mention of a peer would cost another socket and
        // another thread.
        let shared = shared_for_test();
        let peer = SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 9), 2234);
        let first = shared.map_peer(peer).expect("a port");
        let again = shared.map_peer(peer).expect("a port");
        assert_eq!(first, again);
        assert_eq!(shared.peers.lock().unwrap().len(), 1);
    }

    #[test]
    fn messages_without_an_address_are_passed_through_untouched() {
        let shared = shared_for_test();
        let mut body = framed(64, b"room list, or anything else");
        let before = body.clone();
        shared.patch_message(&mut body);
        assert_eq!(body, before);
    }

    #[test]
    fn a_truncated_message_is_left_alone_rather_than_panicking() {
        // Anything can arrive on a socket; indexing past the end of a short
        // message would take the whole session down with it.
        let shared = shared_for_test();
        for cut in 0..24 {
            let mut body = framed(CODE_GET_PEER_ADDRESS, &string_field("peer"));
            body.extend_from_slice(&[9, 113, 0, 203]);
            body.extend_from_slice(&2234u32.to_le_bytes());
            body.truncate(cut);
            shared.patch_message(&mut body);
        }
    }

    #[test]
    fn a_string_length_that_runs_past_the_message_is_refused() {
        let shared = shared_for_test();
        let mut body = framed(CODE_GET_PEER_ADDRESS, &u32::MAX.to_le_bytes());
        body.extend_from_slice(&[9, 113, 0, 203, 0, 0, 0, 0]);
        let before = body.clone();
        shared.patch_message(&mut body);
        assert_eq!(body, before, "a bogus length must not be trusted");
    }

    #[test]
    fn an_unusable_proxy_is_recognised_before_it_is_dialled() {
        let mut proxy = Proxy {
            kind: ProxyKind::Socks5,
            host: "  ".into(),
            port: 1080,
            username: String::new(),
            password: String::new(),
        };
        assert!(!proxy.is_usable(), "a blank host is not a proxy");
        proxy.host = "127.0.0.1".into();
        proxy.port = 0;
        assert!(!proxy.is_usable(), "nor is port zero");
        proxy.port = 1080;
        assert!(proxy.is_usable());
    }
}
