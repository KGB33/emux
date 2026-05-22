use mlua::{Lua, Result as LuaResult, Table};

const EMUX_LIB: &str = include_str!("emux.lua");

fn env_file(lua: &Lua, (path, variable): (String, String)) -> LuaResult<Table> {
    let filter = lua.create_table()?;
    filter.set("__kind", "env_file")?;
    filter.set("path", path)?;
    filter.set("variable", variable)?;

    let filters = lua.create_table()?;
    filters.set(1, filter)?;

    let locator = lua.create_table()?;
    locator.set("filters", filters)?;
    Ok(locator)
}

fn files(lua: &Lua, glob: String) -> LuaResult<Table> {
    let filter = lua.create_table()?;
    filter.set("__kind", "file")?;
    filter.set("glob", glob)?;

    let filters = lua.create_table()?;
    filters.set(1, filter)?;

    let locator = lua.create_table()?;
    locator.set("filters", filters)?;
    Ok(locator)
}

fn regex(lua: &Lua, (target, pattern): (Table, String)) -> LuaResult<Table> {
    let regex_filter = lua.create_table()?;
    regex_filter.set("__kind", "regex")?;
    regex_filter.set("pattern", pattern)?;

    let src_filters: Table = target.get("filters")?;
    let new_filters = lua.create_table()?;
    for f in src_filters.sequence_values::<Table>() {
        new_filters.set(new_filters.raw_len() + 1, f?)?;
    }
    new_filters.set(new_filters.raw_len() + 1, regex_filter)?;

    let locator = lua.create_table()?;
    locator.set("filters", new_filters)?;
    Ok(locator)
}

pub fn load(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();
    globals.set("__emux_env_file", lua.create_function(env_file)?)?;
    globals.set("__emux_files", lua.create_function(files)?)?;
    globals.set("__emux_regex", lua.create_function(regex)?)?;

    let emux: Table = lua.load(EMUX_LIB).set_name("emux").eval()?;
    globals.set("emux", emux)?;

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
        let locator: Table = lua
            .load(r#"emux.l.files("src/**/*.rs")"#)
            .eval()
            .unwrap();
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
