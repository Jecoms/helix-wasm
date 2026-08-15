use std::sync::Arc;

use arc_swap::ArcSwap;
use helix_event::AsyncHook;

use crate::config::Config;
use crate::events;
use crate::handlers::auto_save::AutoSaveHandler;
#[cfg(feature = "dap_lsp")]
use crate::handlers::signature_help::SignatureHelpHandler;

pub use helix_view::handlers::Handlers;

#[cfg(feature = "dap_lsp")]
use self::document_colors::DocumentColorsHandler;

mod auto_save;
#[cfg(feature = "dap_lsp")]
pub mod completion;
mod diagnostics;
#[cfg(feature = "dap_lsp")]
mod document_colors;
#[cfg(feature = "dap_lsp")]
mod signature_help;
mod snippet;

pub fn setup(config: Arc<ArcSwap<Config>>) -> Handlers {
    events::register();

    #[cfg(feature = "dap_lsp")]
    let event_tx = completion::CompletionHandler::new(config).spawn();
    // Without language servers there is no completion backend; sending on a
    // closed channel is a silent no-op, so a dropped receiver suffices.
    #[cfg(not(feature = "dap_lsp"))]
    let event_tx = {
        let _ = &config;
        tokio::sync::mpsc::channel(8).0
    };
    #[cfg(feature = "dap_lsp")]
    let signature_hints = SignatureHelpHandler::new().spawn();
    let auto_save = AutoSaveHandler::new().spawn();
    #[cfg(feature = "dap_lsp")]
    let document_colors = DocumentColorsHandler::default().spawn();

    let handlers = Handlers {
        completions: helix_view::handlers::completion::CompletionHandler::new(event_tx),
        #[cfg(feature = "dap_lsp")]
        signature_hints,
        auto_save,
        #[cfg(feature = "dap_lsp")]
        document_colors,
    };

    helix_view::handlers::register_hooks(&handlers);
    #[cfg(feature = "dap_lsp")]
    {
        completion::register_hooks(&handlers);
        signature_help::register_hooks(&handlers);
        document_colors::register_hooks(&handlers);
    }
    auto_save::register_hooks(&handlers);
    diagnostics::register_hooks(&handlers);
    snippet::register_hooks(&handlers);
    handlers
}
