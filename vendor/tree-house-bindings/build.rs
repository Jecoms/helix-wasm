use std::path::Path;
use std::{env, fs};

fn main() {
    if env::var_os("DISABLED_TS_BUILD").is_some() {
        return;
    }
    let mut config = cc::Build::new();

    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let include_path = manifest_path.join("vendor/include");
    let src_path = manifest_path.join("vendor/src");
    for entry in fs::read_dir(&src_path).unwrap() {
        let entry = entry.unwrap();
        let path = src_path.join(entry.file_name());
        println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
    }

    config
        .flag_if_supported("-std=c11")
        .flag_if_supported("-fvisibility=hidden")
        .flag_if_supported("-Wshadow")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-incompatible-pointer-types")
        .include(&src_path)
        .include(&include_path)
        .define("_POSIX_C_SOURCE", "200112L")
        .define("_DEFAULT_SOURCE", None)
        .warnings(false)
        .file(src_path.join("lib.c"));

    // wasm32-unknown-unknown has no libc and no system headers. Compile the
    // runtime against the freestanding shim headers shared with the grammar
    // build in helix-web, and add the C-side shim implementations (stdio
    // stubs, wctype tables, ...). The allocator itself is provided from Rust
    // (src/wasm_alloc.rs).
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        let sysroot_path = manifest_path.join("../../helix-web/src/wasm-sysroot");
        for file in ["shims.c", "wctype.c"] {
            let path = sysroot_path.join(file);
            println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
            config.file(path);
        }
        config.include(&sysroot_path);
        // Routes tree-sitter's portable/endian.h to the sysroot's endian.h;
        // its platform detection does not know wasm32-unknown-unknown.
        config.define("HAVE_ENDIAN_H", None);
    }

    config.compile("tree-sitter");
}
