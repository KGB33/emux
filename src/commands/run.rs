use mlua::{Error, Lua};
use std::{fs, path::PathBuf, process};

pub fn run_source(source: &str, name: &str) -> Result<(), Error> {
    let lua = Lua::new();
    crate::lua_api::load(&lua)?;
    lua.load(source).set_name(name).exec()?;
    Ok(())
}

pub fn run(file: PathBuf) {
    let source = fs::read_to_string(&file).unwrap_or_else(|err| {
        eprintln!("error: could not read `{}`: {err}", file.display());
        process::exit(1);
    });

    let name = file.display().to_string();
    match file.extension().and_then(|e| e.to_str()) {
        Some("lua") => run_source(&source, &name),
        _ => {
            eprintln!("error: unsupported file type `{}`", file.display());
            process::exit(1);
        }
    }
    .unwrap_or_else(|err| {
        eprintln!("error: {err}");
        process::exit(1);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_emux_config_passes() {
        let src = r#"
            local cfg = {
                ["api-port"] = {
                    locate = { emux.envFile("api/.env", "PORT") },
                    override = emux.int.random,
                },
            }
            return cfg
        "#;
        assert!(run_source(src, "test").is_ok());
    }

    #[test]
    fn all_emux_functions_callable() {
        let src = r#"
            emux.envFile("a/.env", "VAR")
            emux.files("src/**/*.rs")
            emux.regex(emux.files("src/**/*.rs"), "8001")
            local _ = emux.int.random
        "#;
        assert!(run_source(src, "test").is_ok());
    }
}
