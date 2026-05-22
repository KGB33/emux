use colored::Colorize;
use mlua::{Function, Lua, Table, Value};
use std::{fs, path::PathBuf, process};

const FENNEL: &str = include_str!("../fennel-1.6.1.lua");

pub fn run(file: PathBuf) {
    let source = fs::read_to_string(&file).unwrap_or_else(|err| {
        eprintln!("error: could not read `{}`: {err}", file.display());
        process::exit(1);
    });

    let dir = file
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let name = file.display().to_string();

    (|| -> Result<(), Box<dyn std::error::Error>> {
        let lua = Lua::new();
        crate::lua_api::load(&lua)?;
        let cfg_val: Value = match file.extension().and_then(|e| e.to_str()) {
            Some("lua") => lua.load(&source).set_name(&name).eval()?,
            Some("fnl") => {
                let fennel: Table = lua.load(FENNEL).set_name("fennel-1.6.1").eval()?;
                let compile: Function = fennel.get("compileString")?;
                let opts = lua.create_table()?;
                opts.set("filename", name.as_str())?;
                let lua_src: String = compile.call((&*source, opts))?;
                lua.load(&lua_src).set_name(&name).eval()?
            }
            _ => return Err(format!("unsupported file type `{}`", file.display()).into()),
        };
        let cfg = crate::config::cfg_from_lua(cfg_val, &lua)?;
        let entries = crate::config::diff_cfg(&cfg, &dir)?;
        for e in &entries {
            println!("{}", format!("[{}] {}:{}", e.entry_name, e.path.display(), e.line_number).bold().cyan());
            println!("-  {}", color_span(&e.old_line, &e.old_value, |s| s.red()));
            println!("+  {}", color_span(&e.new_line, &e.new_value, |s| s.green()));
        }
        Ok(())
    })()
    .unwrap_or_else(|err| {
        eprintln!("error: {err}");
        process::exit(1);
    });
}

fn color_span(line: &str, span: &str, colorize: impl Fn(&str) -> colored::ColoredString) -> String {
    match line.find(span) {
        Some(pos) => format!("{}{}{}", &line[..pos], colorize(&line[pos..pos + span.len()]), &line[pos + span.len()..]),
        None => line.to_owned(),
    }
}
