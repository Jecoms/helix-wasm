//! Compiles the C side of the statically linked tree-sitter setup on wasm32:
//! the sysroot's libc shims and wctype implementation, plus the parsers of
//! the static grammar set. `cargo check` compiles the runtime's C against
//! the sysroot headers, but only the final wasm link resolves symbols, and
//! this crate produces that final artifact — so the definitions are linked
//! here. The wasm C compiler (the `sysroot/wasm-cc` clang shim) comes from
//! `.cargo/config.toml` via `CC_wasm32_unknown_unknown`; cc-rs picks it up
//! automatically.
//!
//! Grammar sources are fetched at build time, shallow, pinned by revision
//! (the same name/remote/rev entries as helix's own `languages.toml`), into
//! OUT_DIR — nothing is vendored and no fork is involved. The queries for
//! each grammar are vendored in `queries/` (helix's own files; see the
//! README there) and embedded via generated registration code that the
//! `grammars` module includes.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The grammar catalog: (name, git remote, pinned revision). The pins
/// mirror the `[[grammar]]` entries in helix's `languages.toml` at the tag
/// the workspace tracks. To add a grammar: add a row here, vendor its
/// queries under `queries/<name>/`, and keep the name in sync with helix's
/// language configuration (the `tree_sitter_<name>` symbol is derived from
/// it).
///
/// A build links the whole catalog by default; set `HELIX_WEB_GRAMMARS` to
/// a comma-separated subset of these names to slim the bundle (see
/// [`selected_grammars`]).
const GRAMMARS: &[(&str, &str, &str)] = &[
    (
        "c",
        "https://github.com/tree-sitter/tree-sitter-c",
        "7175a6dd5fc1cee660dce6fe23f6043d75af424a",
    ),
    (
        "go",
        "https://github.com/tree-sitter/tree-sitter-go",
        "64457ea6b73ef5422ed1687178d4545c3e91334a",
    ),
    (
        "java",
        "https://github.com/tree-sitter/tree-sitter-java",
        "09d650def6cdf7f479f4b78f595e9ef5b58ce31e",
    ),
    (
        "javascript",
        "https://github.com/tree-sitter/tree-sitter-javascript",
        "f772967f7b7bc7c28f845be2420a38472b16a8ee",
    ),
    (
        "python",
        "https://github.com/tree-sitter/tree-sitter-python",
        "4bfdd9033a2225cc95032ce77066b7aeca9e2efc",
    ),
    (
        "regex",
        "https://github.com/tree-sitter/tree-sitter-regex",
        "e1cfca3c79896ff79842f057ea13e529b66af636",
    ),
    (
        "rust",
        "https://github.com/tree-sitter/tree-sitter-rust",
        "1f63b33efee17e833e0ea29266dd3d713e27e321",
    ),
    (
        "toml",
        "https://github.com/ikatyang/tree-sitter-toml",
        "7cff70bbcbbc62001b465603ca1ea88edd668704",
    ),
];

fn main() {
    // Watch the whole sysroot: the .c files include the sysroot headers
    // (shims.c pulls in stdio.h, stdlib.h, string.h, time.h, unistd.h), and
    // the wasm-cc shim shapes the compile too — a per-file list here
    // under-declared and let local incremental builds link stale objects.
    println!("cargo:rerun-if-changed=../sysroot");
    println!("cargo:rerun-if-changed=queries");
    println!("cargo:rerun-if-env-changed=HELIX_WEB_GRAMMARS");

    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }

    cc::Build::new()
        .file("../sysroot/shims.c")
        .file("../sysroot/wctype.c")
        .compile("helix_web_libc_shims");

    let grammars = selected_grammars();
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    for &(name, remote, rev) in &grammars {
        let src_dir = fetch_grammar(name, remote, rev, &out_dir).join("src");

        let mut build = cc::Build::new();
        build.include(&src_dir).file(src_dir.join("parser.c"));
        let scanner = src_dir.join("scanner.c");
        if scanner.exists() {
            build.file(scanner);
        }
        build.compile(&format!("tree-sitter-{name}"));
    }

    generate_registration(&out_dir, &grammars);
}

/// The catalog entries this build links: all of them by default, or the
/// comma-separated subset named in `HELIX_WEB_GRAMMARS` (e.g.
/// `HELIX_WEB_GRAMMARS=rust,toml`). Unknown and repeated names both fail
/// the build with a message naming the offender — selection can only
/// narrow the pinned set, and a repeat would otherwise surface as a
/// duplicate-definition error in the generated registration code.
fn selected_grammars() -> Vec<(&'static str, &'static str, &'static str)> {
    let selection = std::env::var("HELIX_WEB_GRAMMARS").unwrap_or_default();
    if selection.trim().is_empty() {
        return GRAMMARS.to_vec();
    }
    let mut selected: Vec<(&str, &str, &str)> = Vec::new();
    for name in selection.split(',').map(str::trim) {
        if name.is_empty() {
            continue;
        }
        let entry = *GRAMMARS
            .iter()
            .find(|(catalog_name, _, _)| *catalog_name == name)
            .unwrap_or_else(|| {
                let known: Vec<_> = GRAMMARS.iter().map(|(n, _, _)| *n).collect();
                panic!(
                    "HELIX_WEB_GRAMMARS names unknown grammar '{name}'; \
                     the catalog has: {known:?}"
                )
            });
        assert!(
            !selected.iter().any(|&(seen, _, _)| seen == name),
            "HELIX_WEB_GRAMMARS names grammar '{name}' more than once"
        );
        selected.push(entry);
    }
    selected
}

/// Shallow-fetches the pinned revision of a grammar repository into
/// `<out_dir>/grammar-sources/<name>`, reusing it when it is already at the
/// pin. Requires `git` on PATH, like helix's own grammar fetching.
fn fetch_grammar(name: &str, remote: &str, rev: &str, out_dir: &Path) -> PathBuf {
    let dir = out_dir.join("grammar-sources").join(name);
    if dir.join(".git").exists() {
        let head = git(&dir, &["rev-parse", "HEAD"]);
        if head.as_deref() == Some(rev) {
            return dir;
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "--quiet"]).unwrap();
    git(&dir, &["fetch", "--quiet", "--depth", "1", remote, rev])
        .unwrap_or_else(|| panic!("failed to fetch grammar '{name}' at {rev} from {remote}"));
    git(&dir, &["checkout", "--quiet", rev]).unwrap();
    dir
}

/// Runs a git subcommand in `dir`; gives its trimmed stdout, or None on
/// failure.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run git; the grammar fetch requires git on PATH");
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Generates `grammar_registration.rs`: a `register()` function that hands
/// every grammar in the set, and every vendored query file, to
/// `helix_loader`'s wasm32 static registry. Generated so the grammar list
/// above stays the single source of truth.
fn generate_registration(out_dir: &Path, grammars: &[(&str, &str, &str)]) {
    use std::fmt::Write;

    let queries_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("queries");

    let mut code = String::from(
        "/// Registers the static grammar set and its queries with\n\
         /// `helix_loader`. Generated by build.rs.\n\
         pub fn register() {\n\
         \x20   use helix_wasm::helix_loader::grammar::{register_grammar, register_runtime_file};\n\
         \x20   use tree_house::tree_sitter::Grammar;\n\n\
         \x20   extern \"C\" {\n",
    );
    for &(name, _, _) in grammars {
        let symbol = name.replace('-', "_");
        writeln!(code, "        fn tree_sitter_{symbol}() -> Grammar;").unwrap();
    }
    code.push_str(
        "    }\n\n\
         \x20   // SAFETY: a generated grammar entry point takes no arguments, returns\n\
         \x20   // a pointer to its static TSLanguage, and has no preconditions.\n",
    );
    for &(name, _, _) in grammars {
        let symbol = name.replace('-', "_");
        writeln!(
            code,
            "    register_grammar(\"{name}\", unsafe {{ tree_sitter_{symbol}() }});"
        )
        .unwrap();
    }
    assert_vendored_queries(&queries_dir, grammars);

    // Query registration walks the vendored directory rather than the
    // grammar list: `; inherits:` directives can pull in query-only base
    // languages that have no grammar of their own (javascript inherits from
    // `ecma` and `_javascript`). Queries for unselected grammars register
    // too — a few KB of dead text against computing the inherits closure of
    // an arbitrary subset; helix never reads queries for a language whose
    // grammar did not resolve.
    let mut langs: Vec<_> = std::fs::read_dir(&queries_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    langs.sort();
    for lang_dir in langs {
        let lang = lang_dir.file_name().unwrap().to_str().unwrap().to_owned();
        let mut files: Vec<_> = std::fs::read_dir(&lang_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|file| file.ends_with(".scm"))
            .collect();
        files.sort();
        for file in files {
            let path = lang_dir.join(&file);
            writeln!(
                code,
                "    register_runtime_file(\"{lang}\", \"{file}\", include_str!(r\"{}\"));",
                path.display()
            )
            .unwrap();
        }
    }
    code.push_str("}\n");

    std::fs::write(out_dir.join("grammar_registration.rs"), code).unwrap();
}

/// Asserts that every selected grammar has vendored queries — including the
/// full `; inherits:` closure. A directive can pull in query-only base
/// languages (javascript inherits `ecma`/`_javascript`), and a missing base
/// dir is invisible at runtime: `load_runtime_file` feeds
/// `unwrap_or_default()`, so the inherited part of the query is silently
/// empty and highlighting quietly degrades. Failing the build here turns
/// the manual re-vendor rule in `queries/README.md` into a checked one.
fn assert_vendored_queries(queries_dir: &Path, grammars: &[(&str, &str, &str)]) {
    let mut pending: Vec<(String, String)> = grammars
        .iter()
        .map(|&(name, _, _)| (name.to_owned(), format!("selected grammar '{name}'")))
        .collect();
    let mut seen: std::collections::BTreeSet<String> =
        pending.iter().map(|(name, _)| name.clone()).collect();
    while let Some((lang, needed_by)) = pending.pop() {
        let dir = queries_dir.join(&lang);
        assert!(
            dir.is_dir(),
            "no vendored queries for '{lang}' in queries/ (required by {needed_by}; \
             see queries/README.md for the re-vendor rule)"
        );
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("scm"))
            .collect();
        files.sort();
        for path in files {
            let text = std::fs::read_to_string(&path).unwrap();
            let file = path.file_name().unwrap().to_str().unwrap().to_owned();
            for target in inherits_targets(&text) {
                if seen.insert(target.clone()) {
                    pending.push((target, format!("`; inherits:` in {lang}/{file}")));
                }
            }
        }
    }
}

/// The languages named by `; inherits:` directives in a query file.
/// Mirrors tree-house's `INHERITS_REGEX` (`;+\s*inherits\s*:?\s*([a-z_,()-]+)`)
/// applied per line. Parenthesized names pass through tree-house's language
/// lookup verbatim, never resolve, and read as empty — effectively optional
/// — so they are skipped here rather than asserted.
fn inherits_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        if !line.starts_with(';') {
            continue;
        }
        let rest = line.trim_start_matches(';').trim_start();
        let Some(rest) = rest.strip_prefix("inherits") else {
            continue;
        };
        let rest = rest.trim_start();
        let rest = rest.strip_prefix(':').unwrap_or(rest).trim_start();
        let names: String = rest
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || matches!(c, '_' | ',' | '(' | ')' | '-'))
            .collect();
        for name in names.split(',') {
            if !name.is_empty() && !name.contains(['(', ')']) {
                targets.push(name.to_owned());
            }
        }
    }
    targets
}
