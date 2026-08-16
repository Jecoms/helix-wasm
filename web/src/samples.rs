//! The sample files seeded into the virtual file system at boot.
//!
//! Without them the vfs holds nothing but the vendored runtime files, all
//! under a dotted config path (`.config/helix/runtime/...`): the `space f`
//! file picker opens on a list with nothing a reader would sensibly select,
//! so the tutorial's 13.7 exercise — pick a file, open it in a split — has
//! no file to pick. These also give `:o` and the buffer picker something to
//! work with on a fresh page.

use helix_wasm::helix_stdx::vfs;

const WELCOME: &str = "\
Welcome to the browser build of Helix.

  :tutor            the full Helix tutorial
  space w           the window/split menu (the tutorial calls it Ctrl-w,
                    which most browsers keep for closing the tab)
  :theme gruvbox    one of the bundled color schemes
  :o /example.rs    the other file seeded into this session

Files live in an in-memory file system created fresh on every page load:
:w saves into it, :o reads from it, and a reload wipes it.
";

const EXAMPLE: &str = "\
// The browser build links a small set of tree-sitter grammars, so this
// file is syntax highlighted; `:set-language` switches a buffer over.
fn main() {
    println!(\"{}\", greet(\"Helix\"));
}

fn greet(name: &str) -> String {
    format!(\"Hello from {name}, running on wasm32.\")
}
";

const SAMPLES: &[(&str, &str)] = &[("/welcome.txt", WELCOME), ("/example.rs", EXAMPLE)];

/// Writes every sample file into the vfs, at the boot working directory.
pub fn seed() {
    for &(path, contents) in SAMPLES {
        vfs::write(path, contents.as_bytes()).expect("sample file names are valid vfs paths");
    }
}
