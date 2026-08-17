//! The bundled theme set: seeding glue between the theme TOMLs the build
//! script embedded and the virtual file system helix's wasm32 theme loader
//! reads. The files are helix's own, read from the in-tree port's
//! `helix/runtime/themes/`; build.rs turns the curated selection (its
//! `THEMES` catalog, see `../themes/README.md`) into the `THEMES` table
//! included here.

include!(concat!(env!("OUT_DIR"), "/theme_seed.rs"));

/// Writes every bundled theme into the vfs under the runtime themes
/// directory (`<runtime_dir>/themes/<name>.toml`), where the theme loader
/// searches for `:theme` names.
pub fn seed() {
    let themes_dir = helix_wasm::helix_loader::runtime_dirs()
        .first()
        .expect("wasm32 has exactly one runtime dir")
        .join("themes");
    for &(file, contents) in THEMES {
        helix_wasm::helix_stdx::vfs::write(themes_dir.join(file), contents.as_bytes())
            .expect("bundled theme file names are valid vfs paths");
    }
}
