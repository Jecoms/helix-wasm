use completion::{CompletionEvent, CompletionHandler};
#[cfg(feature = "dap_lsp")]
use helix_event::send_blocking;
use tokio::sync::mpsc::Sender;

#[cfg(feature = "dap_lsp")]
use crate::handlers::lsp::SignatureHelpInvoked;
#[cfg(feature = "dap_lsp")]
use crate::Editor;
use crate::{DocumentId, ViewId};

pub mod completion;
#[cfg(feature = "dap_lsp")]
pub mod dap;
pub mod diagnostics;
#[cfg(feature = "dap_lsp")]
pub mod lsp;

#[derive(Debug)]
pub enum AutoSaveEvent {
    DocumentChanged { save_after: u64 },
    LeftInsertMode,
}

pub struct Handlers {
    // only public because most of the actual implementation is in helix-term right now :/
    pub completions: CompletionHandler,
    #[cfg(feature = "dap_lsp")]
    pub signature_hints: Sender<lsp::SignatureHelpEvent>,
    pub auto_save: Sender<AutoSaveEvent>,
    #[cfg(feature = "dap_lsp")]
    pub document_colors: Sender<lsp::DocumentColorsEvent>,
}

impl Handlers {
    /// Manually trigger completion (c-x)
    pub fn trigger_completions(&self, trigger_pos: usize, doc: DocumentId, view: ViewId) {
        self.completions.event(CompletionEvent::ManualTrigger {
            cursor: trigger_pos,
            doc,
            view,
        });
    }

    #[cfg(feature = "dap_lsp")]
    pub fn trigger_signature_help(&self, invocation: SignatureHelpInvoked, editor: &Editor) {
        let event = match invocation {
            SignatureHelpInvoked::Automatic => {
                if !editor.config().lsp.auto_signature_help {
                    return;
                }
                lsp::SignatureHelpEvent::Trigger
            }
            SignatureHelpInvoked::Manual => lsp::SignatureHelpEvent::Invoked,
        };
        send_blocking(&self.signature_hints, event)
    }
}

pub fn register_hooks(handlers: &Handlers) {
    #[cfg(feature = "dap_lsp")]
    lsp::register_hooks(handlers);
    #[cfg(not(feature = "dap_lsp"))]
    let _ = handlers;
}
