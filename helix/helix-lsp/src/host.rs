//! The seam a host supplies language servers through, where it cannot spawn
//! them: a factory that, asked for a server by *name*, hands back a pair of
//! message channels the [`crate::Client`] then talks JSON-RPC over (one
//! message per item, no framing — see `transport.rs`). What is on the other
//! end is the host's business: in the browser it is a Web Worker reached over
//! `postMessage`, running anything from a scripted responder to a real
//! wasm-compiled server.
//!
//! The factory is consulted by `Client::start` before it tries to resolve
//! the configured `command` as an executable, so a registered name wins over
//! the `command` and `args` in `languages.toml` — those are ignored for it.
//! A name the factory does not know falls through to the executable path
//! (which on wasm32 fails as it always has: there are no subprocesses).
//!
//! Native builds never register a factory, so nothing changes there.

use std::sync::OnceLock;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// One server's transport, from the client's side: the server's messages
/// come in on `incoming`, the client's go out on `outgoing`. Each item is
/// one complete JSON-RPC message.
pub struct Connection {
    pub incoming: UnboundedReceiver<String>,
    pub outgoing: UnboundedSender<String>,
}

/// Asked once per client start, with the language server's configured name;
/// `None` means the host has nothing registered under it. A restart asks
/// again, so the factory hands out a fresh pair each time.
///
/// A plain `fn`, not a closure: the registry is a process-wide static, and
/// a host holding onto non-`Send` handles (JS values) keeps them on its own
/// side of this call.
pub type ConnectionFactory = fn(name: &str) -> Option<Connection>;

static FACTORY: OnceLock<ConnectionFactory> = OnceLock::new();

/// Installs the factory. The first installation wins; later calls are
/// no-ops, so a host may call this as often as it likes (once per
/// registered server, say).
pub fn set_connection_factory(factory: ConnectionFactory) {
    let _ = FACTORY.set(factory);
}

pub(crate) fn connect(name: &str) -> Option<Connection> {
    FACTORY.get().and_then(|factory| factory(name))
}
