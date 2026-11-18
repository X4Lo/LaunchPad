//! Single instance: launching Launchpad again just brings the running window up.
//!
//! A loopback-only socket doubles as lock and "show yourself" signal. Loopback binds
//! don't trigger firewall prompts.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

const PORT: u16 = 47_221;
const MAGIC: &[u8] = b"LAUNCHPAD1\n";

pub enum Instance {
    /// We're the first instance; call `listen` once the event loop is ready.
    Primary(TcpListener),
    /// Another Launchpad is running and has been asked to show its window.
    AlreadyRunning,
    /// The port is taken by something else; run without the guard.
    Unguarded,
}

pub fn acquire() -> Instance {
    let addr = SocketAddr::from(([127, 0, 0, 1], PORT));
    match TcpListener::bind(addr) {
        Ok(l) => Instance::Primary(l),
        Err(_) => match TcpStream::connect_timeout(&addr, Duration::from_millis(300)) {
            Ok(mut s) => {
                let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
                let mut buf = [0u8; MAGIC.len()];
                if s.read_exact(&mut buf).is_ok() && buf == MAGIC {
                    Instance::AlreadyRunning
                } else {
                    Instance::Unguarded
                }
            }
            Err(_) => Instance::Unguarded,
        },
    }
}

/// Accept "show" pings from later launches on a tiny background thread.
pub fn listen(listener: TcpListener, on_ping: impl Fn() + Send + 'static) {
    let _ = std::thread::Builder::new().name("launchpad-instance".into()).stack_size(64 * 1024).spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let _ = stream.write_all(MAGIC);
            on_ping();
        }
    });
}
