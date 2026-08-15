//! Compiles the C support code the statically linked tree-sitter runtime
//! needs on wasm32: the sysroot's libc shims and wctype implementation.
//! `cargo check` compiles the runtime's C against the sysroot headers, but
//! only the final wasm link resolves symbols, and this crate produces that
//! final artifact — so the definitions are linked here. The wasm C compiler
//! (the `sysroot/wasm-cc` clang shim) comes from `.cargo/config.toml` via
//! `CC_wasm32_unknown_unknown`; cc-rs picks it up automatically.

fn main() {
    println!("cargo:rerun-if-changed=../sysroot/shims.c");
    println!("cargo:rerun-if-changed=../sysroot/wctype.c");
    println!("cargo:rerun-if-changed=../sysroot/wctype.h");

    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }

    cc::Build::new()
        .file("../sysroot/shims.c")
        .file("../sysroot/wctype.c")
        .compile("helix_web_libc_shims");
}
