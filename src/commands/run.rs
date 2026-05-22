use mlua::{Error, Function, Lua, Table};
use std::{fs, path::PathBuf, process};

const FENNEL: &str = include_str!("../fennel-1.6.1.lua");

pub fn run_source(source: &str, name: &str) -> Result<(), Error> {
    let lua = Lua::new();
    crate::lua_api::load(&lua)?;
    lua.load(source).set_name(name).exec()?;
    Ok(())
}

pub fn run_fennel_source(source: &str, name: &str) -> Result<(), Error> {
    let lua = Lua::new();
    crate::lua_api::load(&lua)?;
    let fennel: Table = lua.load(FENNEL).set_name("fennel-1.6.1").eval()?;
    let compile: Function = fennel.get("compileString")?;
    let opts = lua.create_table()?;
    opts.set("filename", name)?;
    let lua_src: String = compile.call((source, opts))?;
    lua.load(&lua_src).set_name(name).exec()?;
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
        Some("fnl") => run_fennel_source(&source, &name),
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
                    locate = { emux.l.envFile("api/.env", "PORT") },
                    override = emux.o.randPort,
                },
            }
            return cfg
        "#;
        assert!(run_source(src, "test").is_ok());
    }

    #[test]
    fn all_emux_functions_callable() {
        let src = r#"
            emux.l.envFile("a/.env", "VAR")
            emux.l.files("src/**/*.rs")
            emux.l.regex(emux.l.files("src/**/*.rs"), "8001")
            local _ = emux.o.randPort
        "#;
        assert!(run_source(src, "test").is_ok());
    }

    #[test]
    fn valid_fennel_config_passes() {
        let src = r#"
            (local cfg
              {"api-port"
               {:locate [(emux.l.envFile "api/.env" "PORT")
                         (emux.l.regex (emux.l.files "client/**/*.json") "8001")]
                :override emux.o.randPort}})
            cfg
        "#;
        assert!(run_fennel_source(src, "test").is_ok());
    }
}
