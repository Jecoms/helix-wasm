#[macro_use]
extern crate helix_view;

pub mod application;
pub mod args;
pub mod commands;
pub mod compositor;
pub mod config;
pub mod events;
#[cfg(not(target_arch = "wasm32"))]
pub mod health;
pub mod job;
pub mod keymap;
pub mod ui;

use std::path::Path;

use futures_util::Future;
mod handlers;

use ignore::DirEntry;
use url::Url;

#[cfg(any(windows, target_arch = "wasm32"))]
fn true_color() -> bool {
    true
}

#[cfg(not(any(windows, target_arch = "wasm32")))]
fn true_color() -> bool {
    if matches!(
        std::env::var("COLORTERM").map(|v| matches!(v.as_str(), "truecolor" | "24bit")),
        Ok(true)
    ) {
        return true;
    }

    match termini::TermInfo::from_env() {
        Ok(t) => {
            t.extended_cap("RGB").is_some()
                || t.extended_cap("Tc").is_some()
                || (t.extended_cap("setrgbf").is_some() && t.extended_cap("setrgbb").is_some())
        }
        Err(_) => false,
    }
}

/// Function used for filtering dir entries in the various file pickers.
fn filter_picker_entry(entry: &DirEntry, root: &Path, dedup_symlinks: bool) -> bool {
    // We always want to ignore popular VCS directories, otherwise if
    // `ignore` is turned off, we end up with a lot of noise
    // in our picker.
    if matches!(
        entry.file_name().to_str(),
        Some(".git" | ".pijul" | ".jj" | ".hg" | ".svn")
    ) {
        return false;
    }

    // We also ignore symlinks that point inside the current directory
    // if `dedup_links` is enabled.
    if dedup_symlinks && entry.path_is_symlink() {
        return entry
            .path()
            .canonicalize()
            .ok()
            .is_some_and(|path| !path.starts_with(root));
    }

    true
}

/// Opens URL in external program.
#[cfg(not(target_arch = "wasm32"))]
fn open_external_url_callback(
    url: Url,
) -> impl Future<Output = Result<job::Callback, anyhow::Error>> + Send + 'static {
    let commands = open::commands(url.as_str());
    async {
        for cmd in commands {
            let mut command: tokio::process::Command = cmd.into();
            if command.output().await.is_ok() {
                return Ok(job::Callback::Editor(Box::new(|_| {})));
            }
        }
        Ok(job::Callback::Editor(Box::new(move |editor| {
            editor.set_error("Opening URL in external program failed")
        })))
    }
}

/// There is no external program to hand URLs to on wasm32.
#[cfg(target_arch = "wasm32")]
fn open_external_url_callback(
    url: Url,
) -> impl Future<Output = Result<job::Callback, anyhow::Error>> + Send + 'static {
    let _ = url;
    async {
        Ok(job::Callback::Editor(Box::new(move |editor| {
            editor.set_error("Opening URLs is not supported on wasm32")
        })))
    }
}
