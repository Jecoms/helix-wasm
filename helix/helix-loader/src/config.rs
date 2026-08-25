use std::str::from_utf8;

/// Default built-in languages.toml.
pub fn default_lang_config() -> toml::Value {
    let default_config = include_bytes!("../../languages.toml");
    toml::from_str(from_utf8(default_config).unwrap())
        .expect("Could not parse built-in languages.toml to valid toml")
}

/// User configured languages.toml file, merged with the default config.
pub fn user_lang_config() -> Result<toml::Value, toml::de::Error> {
    let config = [
        crate::config_dir(),
        crate::find_workspace().0.join(".helix"),
    ]
    .into_iter()
    .map(|path| path.join("languages.toml"))
    .filter_map(|file| Some(toml::from_str(&read_lang_config(file)?)))
    .collect::<Result<Vec<_>, _>>()?
    .into_iter()
    .fold(default_lang_config(), |a, b| {
        crate::merge_toml_values(a, b, 3)
    });

    Ok(config)
}

/// One `languages.toml`, or `None` where there is none to read (a missing
/// file is the common case, not an error).
#[cfg(not(target_arch = "wasm32"))]
fn read_lang_config(file: std::path::PathBuf) -> Option<String> {
    std::fs::read_to_string(file).ok()
}

/// wasm32 has no file system; the language config is read from the virtual
/// one, same as `config.toml` and document IO. Both paths above are absolute
/// there, so an embedder can seed either before the editor boots.
#[cfg(target_arch = "wasm32")]
fn read_lang_config(file: std::path::PathBuf) -> Option<String> {
    String::from_utf8(helix_stdx::vfs::read(file).ok()?).ok()
}
