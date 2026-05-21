use mlua::{Error, Lua};
use std::{fs, path::PathBuf, process};

pub fn verify_source(source: &str, name: &str) -> Result<(), Error> {
    let lua = Lua::new();
    lua.load(source).set_name(name).into_function()?;
    Ok(())
}

pub fn run(file: PathBuf) {
    let source = fs::read_to_string(&file).unwrap_or_else(|err| {
        eprintln!("error: could not read `{}`: {err}", file.display());
        process::exit(1);
    });

    let name = file.display().to_string();
    verify_source(&source, &name).unwrap_or_else(|err| {
        eprintln!("error: {err}");
        process::exit(1);
    });

    println!("`{name}` ok");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_lua_passes() {
        assert!(verify_source("local x = 1 + 2", "test").is_ok());
    }

    #[test]
    fn syntax_error_fails() {
        assert!(verify_source("local x = ===", "test").is_err());
    }

    #[test]
    fn empty_file_passes() {
        assert!(verify_source("", "test").is_ok());
    }

    #[test]
    fn valid_function_definition_passes() {
        let src = "local function add(a, b) return a + b end";
        assert!(verify_source(src, "test").is_ok());
    }
}
