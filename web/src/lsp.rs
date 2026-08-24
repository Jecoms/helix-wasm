//! Language servers over a host-supplied message channel (issue #144).
//!
//! The browser cannot spawn the server process helix would normally talk
//! LSP to over stdio, but LSP is JSON-RPC over any byte stream, and a Web
//! Worker's `postMessage` is one. The host page hands [`register_language_server`]
//! a `Worker` (or a `MessagePort`) under the name a `[language-server.<name>]`
//! table in its `languages.toml` uses; when helix launches that server, this
//! module wires the port to the pair of channels `helix_lsp`'s client and
//! transport run on — one JSON-RPC message per `postMessage`, as a string,
//! in each direction. What runs inside the worker is the page's business:
//! a scripted responder (see `www/toy-lsp-worker.js`) or a real
//! wasm-compiled server, with no network involved either way.
//!
//! The `command`/`args` that table also names describe a process, so they
//! are ignored for a registered name; the name is the whole of the match.
//! Unstable, internal to the host page (see crate docs).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use helix_wasm::helix_lsp::host::{self, Connection};
use js_sys::{Function, Reflect};
use tokio::sync::mpsc::unbounded_channel;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// One live connection's handles on this side: the `onmessage` closure (a
/// closure handed to JS is only called for as long as the Rust side keeps
/// it alive) and the flag that tells its outgoing pump to stop.
struct Live {
    /// Never read: held so the JS side has something to call.
    #[allow(dead_code)]
    onmessage: Closure<dyn FnMut(JsValue)>,
    severed: Rc<Cell<bool>>,
}

impl Live {
    /// Cuts the connection off from the port in both directions: dropping
    /// the closure closes the incoming channel (the transport sees
    /// `StreamClosed` and unwinds), and the flag makes the outgoing pump
    /// drop whatever the old client still sends instead of posting it.
    fn sever(self) {
        self.severed.set(true);
    }
}

thread_local! {
    /// The ports the host registered, by server name. Read when helix
    /// launches a server, so a registration only has to precede the first
    /// document that wants the server, not [`crate::start`].
    static PORTS: RefCell<HashMap<String, JsValue>> = RefCell::new(HashMap::new());
    /// The live connections, by server name. Keyed so a restart
    /// (`:lsp-restart`), which connects afresh, severs the old one rather
    /// than leaving two connections on one port: helix shuts the old client
    /// down *after* the new one has attached, and its `shutdown`/`exit`
    /// would otherwise reach the very server the new client just
    /// initialized against (and the `shutdown` response would land on the
    /// new transport as a response without a request).
    static LIVE: RefCell<HashMap<String, Live>> = RefCell::new(HashMap::new());
}

/// Registers `port` — a `Worker` or a `MessagePort`, anything with
/// `postMessage` and an `onmessage` slot — as the transport for the language
/// server helix knows as `name`. Messages are strings, one complete JSON-RPC
/// message each, with no `Content-Length` framing.
///
/// `name` has to match a `[language-server.<name>]` table in the page's
/// `languages.toml` (see the `languages` argument to [`crate::start`]) for
/// helix to ever ask for it; its `command` is ignored. Register before the
/// first document of a language that lists the server is opened — helix
/// launches servers lazily, on the document, and a name it finds nothing
/// under at that moment fails the way an unconfigured server always has.
/// Registering the same name again replaces the port for later launches.
#[wasm_bindgen]
pub fn register_language_server(name: String, port: JsValue) -> Result<(), JsValue> {
    let post_message = Reflect::get(&port, &"postMessage".into())?;
    if !post_message.is_function() {
        return Err(JsValue::from_str(&format!(
            "language server '{name}': the port has no postMessage method"
        )));
    }
    PORTS.with(|ports| ports.borrow_mut().insert(name, port));
    host::set_connection_factory(connect);
    Ok(())
}

/// The factory `helix_lsp`'s client asks for a transport by name. Wires the
/// registered port to a fresh channel pair: the port's `onmessage` feeds the
/// incoming end, and a task drains the outgoing end into `postMessage`.
fn connect(name: &str) -> Option<Connection> {
    let port = PORTS.with(|ports| ports.borrow().get(name).cloned())?;
    let (incoming_tx, incoming) = unbounded_channel::<String>();
    let (outgoing, mut outgoing_rx) = unbounded_channel::<String>();

    // A previous connection under this name (a restart) is cut off first,
    // so nothing its client still sends reaches the port from here on.
    if let Some(old) = LIVE.with(|live| live.borrow_mut().remove(name)) {
        old.sever();
    }

    let server = name.to_owned();
    let onmessage = Closure::wrap(Box::new(move |event: JsValue| {
        let data = Reflect::get(&event, &"data".into()).unwrap_or(JsValue::UNDEFINED);
        let Some(message) = data.as_string() else {
            log::warn!("language server '{server}': dropping a non-string message: {data:?}");
            return;
        };
        // The receiver is gone once the transport has shut down; a late
        // message from the worker is then nobody's business.
        let _ = incoming_tx.send(message);
    }) as Box<dyn FnMut(JsValue)>);
    // Assigning `onmessage` (rather than `addEventListener`) is what starts
    // a `MessagePort` delivering; a `Worker` delivers regardless.
    if let Err(err) = Reflect::set(&port, &"onmessage".into(), onmessage.as_ref()) {
        log::error!("language server '{name}': cannot set onmessage on the port: {err:?}");
        return None;
    }
    let severed = Rc::new(Cell::new(false));
    LIVE.with(|live| {
        live.borrow_mut().insert(
            name.to_owned(),
            Live {
                onmessage,
                severed: severed.clone(),
            },
        )
    });

    let server = name.to_owned();
    let post_message: Function = Reflect::get(&port, &"postMessage".into())
        .ok()?
        .dyn_into()
        .ok()?;
    spawn_local(async move {
        while let Some(message) = outgoing_rx.recv().await {
            if severed.get() {
                // Severed by a newer connection: the port is someone else's
                // now. Dropping the receiver fails the old client's later
                // sends with `StreamClosed`, which is what ends its shutdown.
                log::info!("language server '{server}': dropping a message sent after reconnect");
                break;
            }
            if let Err(err) = post_message.call1(&port, &JsValue::from_str(&message)) {
                // A terminated worker throws here; dropping the receiver
                // is what tells the transport the stream is closed.
                log::error!("language server '{server}': postMessage failed: {err:?}");
                break;
            }
        }
    });

    Some(Connection { incoming, outgoing })
}
