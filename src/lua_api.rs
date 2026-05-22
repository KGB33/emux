use mlua::{Lua, Result as LuaResult, Table, Value};

const EMUX_LIB: &str = include_str!("emux.lua");

fn env_file(_: &Lua, (path, variable): (String, String)) -> LuaResult<()> {
    println!("envFile({path:?}, {variable:?})");
    Ok(())
}

fn files(_: &Lua, glob: String) -> LuaResult<()> {
    println!("files({glob:?})");
    Ok(())
}

fn regex(_: &Lua, (_target, pattern): (Value, String)) -> LuaResult<()> {
    println!("regex({pattern:?})");
    Ok(())
}

pub fn load(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();
    globals.set("__emux_env_file", lua.create_function(env_file)?)?;
    globals.set("__emux_files",    lua.create_function(files)?)?;
    globals.set("__emux_regex",    lua.create_function(regex)?)?;

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
    fn emux_env_file_is_callable() {
        let lua = loaded_lua();
        lua.load(r#"emux.envFile("api/.env", "PORT")"#).exec().unwrap();
    }

    #[test]
    fn emux_files_is_callable() {
        let lua = loaded_lua();
        lua.load(r#"emux.files("src/**/*.rs")"#).exec().unwrap();
    }

    #[test]
    fn emux_regex_is_callable() {
        let lua = loaded_lua();
        lua.load(r#"emux.regex(emux.files("src/**/*.rs"), "8001")"#).exec().unwrap();
    }

    #[test]
    fn emux_int_random_is_random_port_table() {
        let lua = loaded_lua();
        let kind: String = lua.load(r#"emux.int.random.__kind"#).eval().unwrap();
        assert_eq!(kind, "random_port");
    }
}
