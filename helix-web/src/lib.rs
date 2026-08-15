mod backend;
mod events;
mod utils;
#[path = "crossterm/mod.rs"]
mod xtct;

use std::io::Write;

use backend::{spawn_terminal, WebBackend};
use helix_term::{application::Application, args::Args, config::Config};
use wasm_bindgen::prelude::*;
use xtct::XtermJsCrosstermBackend;

include!(concat!(env!("OUT_DIR"), "/tutor.rs"));

const HEADER: &str = r#"===================== Helix wasm disclaimer =====================

This is an alpha port of Helix as a pure wasm application.

Try it out!
Follow the original tutorial below, change the theme (`:theme `),
change the configuration (`:config-open`, `:config-reload`), ...

More info available in the README:
https://github.com/makemeunsee/helix/tree/wasm32/helix-web

================================================================="#;

#[wasm_bindgen(start)]
pub async fn main() {
    utils::set_panic_hook();
    utils::set_logging(log::Level::Debug);

    let terminal = spawn_terminal();
    let mut input_stream = events::event_stream(&terminal);

    helix_loader::initialize_config_file(None);
    // only so that `:log-open` does something; actual logs are found on the JS console
    helix_loader::initialize_log_file(None);

    let config = Config::load_default().unwrap_or_default();

    if let Ok(mut storage) = helix_core::storage::create(".config/helix/runtime/tutor") {
        write!(&mut storage, "{}\n\n{}", HEADER, TUTOR).unwrap_or(());
        // content is staged in memory; the flush commits it to localStorage
        storage.flush().unwrap_or(());
    }

    let mut args = Args::default();
    args.load_tutor = true;

    let backend = WebBackend::new(XtermJsCrosstermBackend::new(&terminal));
    let mut app = Application::new_with_backend(
        args,
        config,
        helix_core::config::default_lang_loader(),
        backend,
    )
    .unwrap();

    app.run(&mut input_stream).await.unwrap();
}
