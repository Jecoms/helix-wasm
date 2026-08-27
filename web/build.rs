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
//! each grammar, the bundled themes and the tutor text are helix's own
//! runtime files, read straight out of the in-tree port at
//! `../helix/runtime/` (see [`helix_runtime_dir`]) and embedded via
//! generated registration code that the `grammars` and `themes` modules
//! include.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A grammar catalog entry: the name helix knows the grammar by (the
/// `[[grammar]]` name in `languages.toml` — the `tree_sitter_<name>` entry
/// point is derived from it), the git remote and pinned revision the parser
/// source is fetched from, and, for repositories that host several grammars
/// (typescript/tsx, markdown/markdown_inline, ocaml), the subdirectory the
/// grammar lives in.
#[derive(Clone, Copy)]
struct GrammarSource {
    name: &'static str,
    remote: &'static str,
    rev: &'static str,
    subpath: Option<&'static str>,
}

const fn grammar(name: &'static str, remote: &'static str, rev: &'static str) -> GrammarSource {
    GrammarSource {
        name,
        remote,
        rev,
        subpath: None,
    }
}

impl GrammarSource {
    const fn subpath(self, subpath: &'static str) -> Self {
        GrammarSource {
            subpath: Some(subpath),
            ..self
        }
    }
}

/// The grammar catalog. The pins mirror the `[[grammar]]` entries in
/// helix's `languages.toml` at the tag the workspace tracks — name, remote,
/// revision and subpath alike. To add a grammar: add a row here, with the
/// name helix's language configuration uses for it (that is what resolves
/// which languages use the grammar, and so which `helix/runtime/queries/`
/// directories the build embeds — see [`query_languages`]), and record its
/// license attribution in `NOTICE.md` (also on pin bumps).
///
/// Only grammars whose external scanner is C (`src/scanner.c`) or that have
/// none can be linked: the wasm C toolchain here has no C++ sysroot, so a
/// grammar shipping `scanner.cc` at helix's pin (php, ruby, yaml, cmake at
/// 25.07.1) fails the build with a message saying so rather than being
/// added. `gitcommit` is C but is left out on purpose: its generated
/// `parser.c` (a 74k-line lexer) takes clang tens of minutes and gigabytes
/// to compile for wasm32.
///
/// A build links [`DEFAULT_GRAMMARS`] unless `HELIX_WEB_GRAMMARS` says
/// otherwise (see [`selected_grammars`]); the rest of the catalog is opt-in.
const GRAMMARS: &[GrammarSource] = &[
    grammar(
        "bash",
        "https://github.com/tree-sitter/tree-sitter-bash",
        "487734f87fd87118028a65a4599352fa99c9cde8",
    ),
    grammar(
        "c",
        "https://github.com/tree-sitter/tree-sitter-c",
        "7175a6dd5fc1cee660dce6fe23f6043d75af424a",
    ),
    grammar(
        "c-sharp",
        "https://github.com/tree-sitter/tree-sitter-c-sharp",
        "b5eb5742f6a7e9438bee22ce8026d6b927be2cd7",
    ),
    grammar(
        "clojure",
        "https://github.com/sogaiu/tree-sitter-clojure",
        "e57c569ae332ca365da623712ae1f50f84daeae2",
    ),
    grammar(
        "cpp",
        "https://github.com/tree-sitter/tree-sitter-cpp",
        "56455f4245baf4ea4e0881c5169de69d7edd5ae7",
    ),
    grammar(
        "css",
        "https://github.com/tree-sitter/tree-sitter-css",
        "769203d0f9abe1a9a691ac2b9fe4bb4397a73c51",
    ),
    grammar(
        "diff",
        "https://github.com/the-mikedavis/tree-sitter-diff",
        "fd74c78fa88a20085dbc7bbeaba066f4d1692b63",
    ),
    grammar(
        "dockerfile",
        "https://github.com/camdencheek/tree-sitter-dockerfile",
        "087daa20438a6cc01fa5e6fe6906d77c869d19fe",
    ),
    grammar(
        "elixir",
        "https://github.com/elixir-lang/tree-sitter-elixir",
        "02a6f7fd4be28dd94ee4dd2ca19cb777053ea74e",
    ),
    grammar(
        "git-config",
        "https://github.com/the-mikedavis/tree-sitter-git-config",
        "9c2a1b7894e6d9eedfe99805b829b4ecd871375e",
    ),
    grammar(
        "git-rebase",
        "https://github.com/the-mikedavis/tree-sitter-git-rebase",
        "d8a4207ebbc47bd78bacdf48f883db58283f9fd8",
    ),
    grammar(
        "gitattributes",
        "https://github.com/mtoohey31/tree-sitter-gitattributes",
        "3dd50808e3096f93dccd5e9dc7dc3dba2eb12dc4",
    ),
    grammar(
        "gitignore",
        "https://github.com/shunsambongi/tree-sitter-gitignore",
        "f4685bf11ac466dd278449bcfe5fd014e94aa504",
    ),
    grammar(
        "go",
        "https://github.com/tree-sitter/tree-sitter-go",
        "64457ea6b73ef5422ed1687178d4545c3e91334a",
    ),
    grammar(
        "haskell",
        "https://github.com/tree-sitter/tree-sitter-haskell",
        "0975ef72fc3c47b530309ca93937d7d143523628",
    ),
    grammar(
        "hcl",
        "https://github.com/tree-sitter-grammars/tree-sitter-hcl",
        "9e3ec9848f28d26845ba300fd73c740459b83e9b",
    ),
    grammar(
        "heex",
        "https://github.com/phoenixframework/tree-sitter-heex",
        "f6b83f305a755cd49cf5f6a66b2b789be93dc7b9",
    ),
    grammar(
        "html",
        "https://github.com/tree-sitter/tree-sitter-html",
        "cbb91a0ff3621245e890d1c50cc811bffb77a26b",
    ),
    grammar(
        "ini",
        "https://github.com/justinmk/tree-sitter-ini",
        "32b31863f222bf22eb43b07d4e9be8017e36fb31",
    ),
    grammar(
        "java",
        "https://github.com/tree-sitter/tree-sitter-java",
        "09d650def6cdf7f479f4b78f595e9ef5b58ce31e",
    ),
    grammar(
        "javascript",
        "https://github.com/tree-sitter/tree-sitter-javascript",
        "f772967f7b7bc7c28f845be2420a38472b16a8ee",
    ),
    grammar(
        "json",
        "https://github.com/tree-sitter/tree-sitter-json",
        "73076754005a460947cafe8e03a8cf5fa4fa2938",
    ),
    grammar(
        "kotlin",
        "https://github.com/fwcd/tree-sitter-kotlin",
        "c4ddea359a7ff4d92360b2efcd6cfce5dc25afe6",
    ),
    grammar(
        "lua",
        "https://github.com/tree-sitter-grammars/tree-sitter-lua",
        "88e446476a1e97a8724dff7a23e2d709855077f2",
    ),
    grammar(
        "make",
        "https://github.com/alemuller/tree-sitter-make",
        "a4b9187417d6be349ee5fd4b6e77b4172c6827dd",
    ),
    grammar(
        "markdown",
        "https://github.com/tree-sitter-grammars/tree-sitter-markdown",
        "62516e8c78380e3b51d5b55727995d2c511436d8",
    )
    .subpath("tree-sitter-markdown"),
    grammar(
        "markdown_inline",
        "https://github.com/tree-sitter-grammars/tree-sitter-markdown",
        "62516e8c78380e3b51d5b55727995d2c511436d8",
    )
    .subpath("tree-sitter-markdown-inline"),
    grammar(
        "nix",
        "https://github.com/nix-community/tree-sitter-nix",
        "1b69cf1fa92366eefbe6863c184e5d2ece5f187d",
    ),
    grammar(
        "ocaml",
        "https://github.com/tree-sitter/tree-sitter-ocaml",
        "9965d208337d88bbf1a38ad0b0fe49e5f5ec9677",
    )
    .subpath("ocaml"),
    grammar(
        "python",
        "https://github.com/tree-sitter/tree-sitter-python",
        "4bfdd9033a2225cc95032ce77066b7aeca9e2efc",
    ),
    grammar(
        "regex",
        "https://github.com/tree-sitter/tree-sitter-regex",
        "e1cfca3c79896ff79842f057ea13e529b66af636",
    ),
    grammar(
        "rust",
        "https://github.com/tree-sitter/tree-sitter-rust",
        "1f63b33efee17e833e0ea29266dd3d713e27e321",
    ),
    grammar(
        "scala",
        "https://github.com/tree-sitter/tree-sitter-scala",
        "7891815f42dca9ed6aeb464c2edc39d479ab965c",
    ),
    grammar(
        "scss",
        "https://github.com/serenadeai/tree-sitter-scss",
        "c478c6868648eff49eb04a4df90d703dc45b312a",
    ),
    grammar(
        "sql",
        "https://github.com/DerekStride/tree-sitter-sql",
        "b9d109588d5b5ed986c857464830c2f0bef53f18",
    ),
    grammar(
        "swift",
        "https://github.com/alex-pinkus/tree-sitter-swift",
        "57c1c6d6ffa1c44b330182d41717e6fe37430704",
    ),
    grammar(
        "toml",
        "https://github.com/ikatyang/tree-sitter-toml",
        "7cff70bbcbbc62001b465603ca1ea88edd668704",
    ),
    grammar(
        "tsx",
        "https://github.com/tree-sitter/tree-sitter-typescript",
        "b1bf4825d9eaa0f3bdeb1e52f099533328acfbdf",
    )
    .subpath("tsx"),
    grammar(
        "typescript",
        "https://github.com/tree-sitter/tree-sitter-typescript",
        "b1bf4825d9eaa0f3bdeb1e52f099533328acfbdf",
    )
    .subpath("typescript"),
    grammar(
        "xml",
        "https://github.com/RenjiSann/tree-sitter-xml",
        "48a7c2b6fb9d515577e115e6788937e837815651",
    ),
    grammar(
        "zig",
        "https://github.com/tree-sitter-grammars/tree-sitter-zig",
        "eb7d58c2dc4fbeea4745019dee8df013034ae66b",
    ),
];

/// The grammars a build links when `HELIX_WEB_GRAMMARS` is unset — what the
/// published tarball, the demo and every embedder who does not ask for more
/// get. A size budget, not a quality ranking: the eight languages the
/// bundle has always shipped plus the config/data/docs grammars a working
/// developer opens most (json, markdown and its inline half, bash, html,
/// css) and typescript/tsx, all of them cheap. The rest of the catalog is
/// opt-in — a grammar's generated parser is anywhere from a few KB to 5 MB
/// of wasm, and the seven biggest (kotlin, ocaml, c-sharp, haskell, scala,
/// cpp, swift) alone would triple the bundle. `HELIX_WEB_GRAMMARS=full`
/// links everything.
const DEFAULT_GRAMMARS: &[&str] = &[
    "bash",
    "c",
    "css",
    "go",
    "html",
    "java",
    "javascript",
    "json",
    "markdown",
    "markdown_inline",
    "python",
    "regex",
    "rust",
    "toml",
    "tsx",
    "typescript",
];

/// The theme catalog: file stems under `helix/runtime/themes/`. A curated
/// set — helix ships far more than a browser bundle wants to carry — chosen
/// to cover distinct palettes, including two light themes
/// (`catppuccin_latte`, `onelight`). Unlike the query set, which the
/// `; inherits:` closure of [`GRAMMARS`] derives, this one is a judgement
/// call and so is written out.
///
/// A theme that `inherits` from another must have its parent listed here
/// too; [`generate_theme_seed`] asserts that closure, because an unresolved
/// parent surfaces at runtime only as a theme that silently refuses to load.
///
/// Most of these are helix's own MPL-2.0 files, but not all: when adding
/// one, check `helix/runtime/themes/licenses/` for a matching license file
/// and record it in `NOTICE.md` if there is one.
const THEME_CATALOG: &[&str] = &[
    "catppuccin_latte",
    "catppuccin_mocha",
    "dracula",
    "everforest_dark",
    "gruvbox",
    "nord",
    "onedark",
    "onelight",
    "rose_pine",
    "tokyonight",
];

/// helix's runtime directory: `helix/runtime/` in this same workspace, one
/// level up from this crate. The port is an in-tree path dependency, so the
/// queries and themes this script embeds are read from here rather than
/// copied into `web/` — a copy could only go stale against the release the
/// tree carries. (`src/session.rs` embeds `helix/runtime/tutor` the same
/// way, with a plain `include_str!`.)
fn helix_runtime_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the web crate sits one level below the workspace root")
        .join("helix")
        .join("runtime")
}

fn main() {
    // Watch the whole sysroot: the .c files include the sysroot headers
    // (shims.c pulls in stdio.h, stdlib.h, string.h, time.h, unistd.h), and
    // the wasm-cc shim shapes the compile too — a per-file list here
    // under-declared and let local incremental builds link stale objects.
    println!("cargo:rerun-if-changed=../sysroot");
    println!("cargo:rerun-if-env-changed=HELIX_WEB_GRAMMARS");
    // The watches on helix's runtime files are emitted per embedded item by
    // the generators below, not as a blanket watch on `../helix/runtime`:
    // cargo walks a watched directory recursively, and upstream's queries
    // tree alone is 279 languages of files this bundle never reads.

    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }

    cc::Build::new()
        .file("../sysroot/shims.c")
        .file("../sysroot/wctype.c")
        .compile("helix_web_libc_shims");

    for name in DEFAULT_GRAMMARS {
        assert!(
            GRAMMARS.iter().any(|source| source.name == *name),
            "DEFAULT_GRAMMARS names '{name}', which the GRAMMARS catalog does not have"
        );
    }
    let grammars = selected_grammars();
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    // A manifest of what this build links, one name per line in catalog
    // order, headed by the selection that produced it. The release workflow
    // copies it into each tarball as GRAMMARS.txt: the two tiers' package.json
    // are byte-identical, so nothing else in an extracted tree says which
    // grammars it carries.
    let manifest = format!(
        "# HELIX_WEB_GRAMMARS={}\n{}\n",
        std::env::var("HELIX_WEB_GRAMMARS")
            .ok()
            .filter(|selection| !selection.trim().is_empty())
            .unwrap_or_else(|| "default".to_owned()),
        grammars
            .iter()
            .map(|source| source.name)
            .collect::<Vec<_>>()
            .join("\n")
    );
    std::fs::write(out_dir.join("GRAMMARS.txt"), manifest).unwrap();
    for source in &grammars {
        let name = source.name;
        let mut src_dir = fetch_grammar(source, &out_dir);
        if let Some(subpath) = source.subpath {
            src_dir = src_dir.join(subpath);
        }
        let src_dir = src_dir.join("src");

        let mut build = cc::Build::new();
        build.include(&src_dir).file(src_dir.join("parser.c"));
        let scanner = src_dir.join("scanner.c");
        if scanner.exists() {
            build.file(scanner);
        } else if let Some(cxx) = ["scanner.cc", "scanner.cpp"]
            .into_iter()
            .find(|file| src_dir.join(file).exists())
        {
            // Left out, the scanner would surface as undefined
            // `tree_sitter_<name>_external_scanner_*` symbols at the final
            // wasm link, far from the grammar that caused it.
            panic!(
                "grammar '{name}' has a C++ external scanner ({cxx}) at its pin; \
                 the wasm C toolchain has no C++ sysroot, so only grammars with a \
                 C scanner (or none) can be linked"
            );
        }
        build.compile(&format!("tree-sitter-{name}"));
    }

    generate_registration(&out_dir, &grammars);
    generate_theme_seed(&out_dir);
}

/// The catalog entries this build links: [`DEFAULT_GRAMMARS`] when
/// `HELIX_WEB_GRAMMARS` is unset, otherwise the union of what its
/// comma-separated items name — a grammar from the catalog, or one of two
/// aliases: `default` for the default set and `full` for the whole catalog.
/// `HELIX_WEB_GRAMMARS=rust,toml` slims the bundle to two grammars;
/// `HELIX_WEB_GRAMMARS=default,kotlin` adds one to the default set;
/// `HELIX_WEB_GRAMMARS=full` links everything. An unknown name fails the
/// build with a message naming it; a name that comes up twice (spelled out
/// and via an alias, say) is linked once. Repeats are tolerated because of
/// the aliases: `default,rust` is a legitimate way to say "the default set,
/// and make sure rust is in it", whereas the strict panic this had before
/// them (PR #39) only had bare names to guard, where a repeat could only be
/// a typo. The result keeps catalog order, so the same selection spelled
/// two ways builds the same registration code.
fn selected_grammars() -> Vec<GrammarSource> {
    let selection = std::env::var("HELIX_WEB_GRAMMARS").unwrap_or_default();
    let selection = if selection.trim().is_empty() {
        "default".to_owned()
    } else {
        selection
    };
    let mut names: Vec<&str> = Vec::new();
    for item in selection.split(',').map(str::trim) {
        let expanded: Vec<&str> = match item {
            "" => continue,
            "default" => DEFAULT_GRAMMARS.to_vec(),
            "full" => GRAMMARS.iter().map(|source| source.name).collect(),
            name => {
                assert!(
                    GRAMMARS.iter().any(|source| source.name == name),
                    "HELIX_WEB_GRAMMARS names unknown grammar '{name}'; the catalog \
                     has: {:?} (plus the aliases 'default' and 'full')",
                    GRAMMARS
                        .iter()
                        .map(|source| source.name)
                        .collect::<Vec<_>>()
                );
                vec![name]
            }
        };
        for name in expanded {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    GRAMMARS
        .iter()
        .filter(|source| names.contains(&source.name))
        .copied()
        .collect()
}

/// Shallow-fetches the pinned revision of a grammar repository into
/// `<out_dir>/grammar-sources/<name>`, reusing it when it is already at the
/// pin. Requires `git` on PATH, like helix's own grammar fetching. Gives the
/// checkout root; a grammar with a `subpath` lives below it. Two grammars
/// sharing a repository (typescript/tsx) get two checkouts, keyed by name,
/// so each can be checked against its own pin independently.
fn fetch_grammar(source: &GrammarSource, out_dir: &Path) -> PathBuf {
    let GrammarSource {
        name, remote, rev, ..
    } = *source;
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
/// every grammar in the set, and every query file those grammars need, to
/// `helix_loader`'s wasm32 static registry. Generated so the grammar list
/// above stays the single source of truth.
fn generate_registration(out_dir: &Path, grammars: &[GrammarSource]) {
    use std::fmt::Write;

    let queries_dir = helix_runtime_dir().join("queries");

    let mut code = String::from(
        "/// Registers the static grammar set and its queries with\n\
         /// `helix_loader`. Generated by build.rs.\n\
         pub fn register() {\n\
         \x20   use helix_wasm::helix_loader::grammar::{register_grammar, register_runtime_file};\n\
         \x20   use tree_house::tree_sitter::Grammar;\n\n\
         \x20   extern \"C\" {\n",
    );
    for source in grammars {
        let symbol = source.name.replace('-', "_");
        writeln!(code, "        fn tree_sitter_{symbol}() -> Grammar;").unwrap();
    }
    code.push_str(
        "    }\n\n\
         \x20   // SAFETY: a generated grammar entry point takes no arguments, returns\n\
         \x20   // a pointer to its static TSLanguage, and has no preconditions.\n",
    );
    for source in grammars {
        let name = source.name;
        let symbol = name.replace('-', "_");
        writeln!(
            code,
            "    register_grammar(\"{name}\", unsafe {{ tree_sitter_{symbol}() }});"
        )
        .unwrap();
    }
    // The registered set is every language that uses a selected grammar
    // (`markdown_inline` serves `markdown.inline`; `json` serves `json` and
    // `jsonc`) plus its `; inherits:` closure — a directive can pull in
    // query-only base languages that have no grammar of their own
    // (javascript inherits from `ecma` and `_javascript`), and those have
    // to register too.
    //
    // Closing over the *selection* means `HELIX_WEB_GRAMMARS` narrows the
    // queries along with the grammars. Reading from `helix/runtime/queries/`
    // is what makes that the only sane choice — upstream carries hundreds of
    // languages, so there is no "just embed the directory" any more — and it
    // costs nothing: helix never reads queries for a language whose grammar
    // did not resolve, so the dropped files were dead text.
    for lang in query_languages(&helix_runtime_dir(), grammars) {
        let lang_dir = queries_dir.join(&lang);
        // Watch the language directory, not each file: this catches a query
        // file appearing or disappearing upstream as well as its contents
        // changing.
        println!("cargo:rerun-if-changed={}", lang_dir.display());
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

/// Generates `theme_seed.rs`: a `THEMES` table of every theme in
/// [`THEME_CATALOG`], read from `helix/runtime/themes/` and embedded for the
/// frontend to seed into the virtual file system at startup (see
/// `src/themes.rs`). Asserts the `inherits` closure: a theme whose parent is
/// neither in the catalog nor built in would resolve at runtime to "File not
/// found" and the theme would silently refuse to load.
fn generate_theme_seed(out_dir: &Path) {
    use std::fmt::Write;

    let themes_dir = helix_runtime_dir().join("themes");
    let names: std::collections::BTreeSet<&str> = THEME_CATALOG.iter().copied().collect();
    assert_eq!(
        names.len(),
        THEME_CATALOG.len(),
        "the theme catalog names a theme more than once"
    );

    let mut code = String::from(
        "/// The bundled theme set: (file name, contents). Generated by build.rs\n\
         /// from THEME_CATALOG, read out of helix/runtime/themes/.\n\
         const THEMES: &[(&str, &str)] = &[\n",
    );
    for name in &names {
        let path = themes_dir.join(format!("{name}.toml"));
        println!("cargo:rerun-if-changed={}", path.display());
        let text = std::fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!(
                "cannot read theme '{name}' at {} ({err}); THEME_CATALOG in \
                 build.rs names it (see themes/README.md)",
                path.display()
            )
        });
        if let Some(parent) = inherits_parent(&text) {
            assert!(
                names.contains(parent.as_str())
                    || matches!(parent.as_str(), "default" | "base16_default"),
                "theme '{name}' inherits '{parent}', which THEME_CATALOG in \
                 build.rs does not list (see themes/README.md for the closure rule)"
            );
        }
        writeln!(
            code,
            "    (\"{name}.toml\", include_str!(r\"{}\")),",
            path.display()
        )
        .unwrap();
    }
    code.push_str("];\n");

    std::fs::write(out_dir.join("theme_seed.rs"), code).unwrap();
}

/// The parent theme named by a top-level `inherits = "<name>"` line, if
/// any. A line-based parse, not a TOML one: the build script has no toml
/// dependency, and helix's theme files keep `inherits` as a plain
/// top-level assignment. The equivalent quoted-key forms
/// (`"inherits" = ...`, `'inherits' = ...`) are accepted too, so a style
/// change upstream can't silently skip the closure assert.
fn inherits_parent(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim_start();
        let Some(rest) = ["inherits", "\"inherits\"", "'inherits'"]
            .into_iter()
            .find_map(|key| line.strip_prefix(key))
        else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(value) = rest.strip_prefix('=') else {
            continue;
        };
        let value = value.trim().trim_matches(['"', '\'']);
        if !value.is_empty() {
            return Some(value.to_owned());
        }
    }
    None
}

/// The sorted set of languages whose queries this build embeds: every
/// language in helix's `languages.toml` that uses one of the selected
/// grammars, plus the full `; inherits:` closure over them.
///
/// Languages and grammars are not one-to-one: a language names its grammar
/// with an explicit `grammar = "..."` when the two differ (`markdown.inline`
/// uses `markdown_inline`, `git-commit` uses `gitcommit`) and several
/// languages can share one (`json` and `jsonc`, `javascript` and `jsx`),
/// while queries are keyed by language. helix reads them by language name at
/// runtime, so that is the set that has to register — a language whose
/// grammar is linked but whose queries are absent is a file that opens
/// with no highlighting. Deriving the set from `languages.toml` — rather
/// than listing it — is why adding a grammar needs no second edit; it also
/// catches a catalog name that no language uses (a typo), which fails the
/// build.
///
/// A directive can pull in query-only base languages (javascript inherits
/// `ecma`/`_javascript`), and a missing base is invisible at runtime:
/// `load_runtime_file` feeds `unwrap_or_default()`, so the inherited part of
/// the query is silently empty and highlighting quietly degrades. The
/// closure finds those bases in `helix/runtime/queries/`, and a base upstream
/// has no queries for fails the build instead. A *language* upstream has no
/// queries for is skipped, because that is a legitimate state (helix reads
/// empty queries for it natively too).
fn query_languages(runtime_dir: &Path, grammars: &[GrammarSource]) -> Vec<String> {
    let queries_dir = runtime_dir.join("queries");
    let languages_toml = runtime_dir
        .parent()
        .expect("helix/runtime sits below helix/")
        .join("languages.toml");
    println!("cargo:rerun-if-changed={}", languages_toml.display());
    let language_grammars = language_grammars(&std::fs::read_to_string(&languages_toml).unwrap());

    let mut pending: Vec<(String, String)> = Vec::new();
    for source in grammars {
        let name = source.name;
        let mut users = language_grammars
            .iter()
            .filter(|(_, grammar)| grammar == name)
            .map(|(language, _)| language)
            .peekable();
        assert!(
            users.peek().is_some(),
            "no language in {} uses grammar '{name}'; the GRAMMARS catalog in \
             build.rs names it (see queries/README.md)",
            languages_toml.display()
        );
        for language in users {
            if queries_dir.join(language).is_dir() {
                pending.push((language.clone(), format!("selected grammar '{name}'")));
            }
        }
    }
    let mut seen: std::collections::BTreeSet<String> =
        pending.iter().map(|(name, _)| name.clone()).collect();
    while let Some((lang, needed_by)) = pending.pop() {
        let dir = queries_dir.join(&lang);
        assert!(
            dir.is_dir(),
            "no queries for '{lang}' at {} (required by {needed_by}; \
             see queries/README.md)",
            dir.display()
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
    seen.into_iter().collect()
}

/// Every `[[language]]` in helix's `languages.toml` as (language name,
/// grammar name): the block's `grammar = "..."` when it has one, else its
/// `name`, which is how helix itself resolves a language's grammar. A
/// line-based parse, not a TOML one — the build script has no toml
/// dependency, and upstream keeps both keys as plain top-level assignments
/// in the block (sub-tables such as `[language.debugger]`, which carry a
/// `name` of their own, start a new header and so end the block).
fn language_grammars(text: &str) -> Vec<(String, String)> {
    let mut languages = Vec::new();
    let mut current: Option<(Option<String>, Option<String>)> = None;
    let flush = |current: &mut Option<(Option<String>, Option<String>)>,
                 languages: &mut Vec<(String, String)>| {
        if let Some((Some(name), grammar)) = current.take() {
            let grammar = grammar.unwrap_or_else(|| name.clone());
            languages.push((name, grammar));
        }
    };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            flush(&mut current, &mut languages);
            if line == "[[language]]" {
                current = Some((None, None));
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some((name, grammar)) = current.as_mut() else {
            continue;
        };
        // The value is a quoted string; anything after the closing quote is
        // a trailing comment.
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|rest| rest.split_once('"'))
            .map_or(value, |(quoted, _)| quoted)
            .to_owned();
        match key.trim() {
            "name" => *name = Some(value),
            "grammar" => *grammar = Some(value),
            _ => {}
        }
    }
    flush(&mut current, &mut languages);
    languages
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
