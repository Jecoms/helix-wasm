//! Trimmed for the wasm32 stub: there is never a language-server process to
//! pump bytes to, so only [`Payload`] (referenced by `Client`'s send path)
//! survives from the upstream module.
use crate::{jsonrpc, Result};
use serde_json::Value;
use tokio::sync::mpsc::Sender;

// Nothing reads the payload contents any more — they used to feed the
// server's stdin pump — but `Client`'s send path still constructs them.
#[allow(dead_code)]
#[derive(Debug)]
pub enum Payload {
    Request {
        chan: Sender<Result<Value>>,
        value: jsonrpc::MethodCall,
    },
    Notification(jsonrpc::Notification),
    Response(jsonrpc::Output),
}
