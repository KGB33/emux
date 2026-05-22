use mlua::{Function, Lua, Result as LuaResult, Table};

const EMUX_LIB: &str = include_str!("emux.fnl");
const FENNEL: &str = include_str!("fennel-1.6.1.lua");

pub fn compile_fennel(lua: &Lua, source: &str, name: &str) -> LuaResult<String> {
    let fennel: Table = lua.load(FENNEL).set_name("fennel-1.6.1").eval()?;
    let compile: Function = fennel.get("compileString")?;
    let opts = lua.create_table()?;
    opts.set("filename", name)?;
    compile.call((source, opts))
}

pub fn load(lua: &Lua) -> LuaResult<()> {
    let compiled = compile_fennel(lua, EMUX_LIB, "emux.fnl")?;
    let emux: Table = lua.load(&compiled).set_name("emux").eval()?;
    lua.globals().set("emux", emux)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded_lua() -> Lua {
        let lua = Lua::new();
        load(&lua).unwrap();
        lua
    }

    #[test]
    fn load_succeeds() {
        loaded_lua();
    }

    #[test]
    fn emux_l_env_file_returns_locator() {
        let lua = loaded_lua();
        let locator: Table = lua
            .load(r#"emux.l.envFile("api/.env", "PORT")"#)
            .eval()
            .unwrap();
        let filters: Table = locator.get("filters").unwrap();
        let filter: Table = filters.get(1).unwrap();
        assert_eq!(filter.get::<String>("__kind").unwrap(), "env_file");
        assert_eq!(filter.get::<String>("path").unwrap(), "api/.env");
        assert_eq!(filter.get::<String>("variable").unwrap(), "PORT");
    }

    #[test]
    fn emux_l_files_returns_locator() {
        let lua = loaded_lua();
        let locator: Table = lua.load(r#"emux.l.files("src/**/*.rs")"#).eval().unwrap();
        let filters: Table = locator.get("filters").unwrap();
        let filter: Table = filters.get(1).unwrap();
        let kind: String = filter.get("__kind").unwrap();
        let glob: String = filter.get("glob").unwrap();
        assert_eq!(kind, "file");
        assert_eq!(glob, "src/**/*.rs");
    }

    #[test]
    fn emux_l_regex_returns_locator_with_both_filters() {
        let lua = loaded_lua();
        let locator: Table = lua
            .load(r#"emux.l.regex(emux.l.files("src/**/*.rs"), "8001")"#)
            .eval()
            .unwrap();
        let filters: Table = locator.get("filters").unwrap();
        let file_filter: Table = filters.get(1).unwrap();
        let regex_filter: Table = filters.get(2).unwrap();
        assert_eq!(file_filter.get::<String>("__kind").unwrap(), "file");
        assert_eq!(file_filter.get::<String>("glob").unwrap(), "src/**/*.rs");
        assert_eq!(regex_filter.get::<String>("__kind").unwrap(), "regex");
        assert_eq!(regex_filter.get::<String>("pattern").unwrap(), "8001");
    }

    #[test]
    fn emux_o_rand_port_is_random_port_table() {
        let lua = loaded_lua();
        let kind: String = lua.load(r#"emux.o.randPort.__kind"#).eval().unwrap();
        assert_eq!(kind, "random_port");
    }
}
