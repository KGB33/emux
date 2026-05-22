mod locator;
mod overrider;

pub use locator::{Filter, Locator};
pub use overrider::Overrider;

use std::collections::HashMap;

use mlua::{FromLua, Lua, Result as LuaResult, Table, Value};

pub type Cfg = HashMap<String, ConfigEntry>;

#[derive(Debug)]
pub struct ConfigEntry {
    pub locate: Vec<Locator>,
    pub overrider: Overrider, // `override` is a reserved keyword
}

/// Deserialize a `Cfg` from the value returned by evaluating a config file.
pub fn cfg_from_lua(value: Value, lua: &Lua) -> LuaResult<Cfg> {
    let table = expect_table(value, "Cfg")?;
    let mut map = HashMap::new();
    for pair in table.pairs::<String, Value>() {
        let (key, val) = pair?;
        map.insert(key, ConfigEntry::from_lua(val, lua)?);
    }
    Ok(map)
}

impl FromLua for ConfigEntry {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        let table = expect_table(value, "ConfigEntry")?;

        let locate_table = expect_table(table.get("locate")?, "locate")?;
        let len = locate_table.raw_len();
        let mut locate = Vec::with_capacity(len as usize);
        for i in 1..=len {
            locate.push(Locator::from_lua(locate_table.raw_get(i)?, lua)?);
        }

        let overrider = Overrider::from_lua(table.get("override")?, lua)?;
        Ok(ConfigEntry { locate, overrider })
    }
}

pub(crate) fn expect_table(value: Value, context: &str) -> LuaResult<Table> {
    match value {
        Value::Table(t) => Ok(t),
        other => Err(mlua::Error::FromLuaConversionError {
            from: other.type_name(),
            to: context.to_string(),
            message: Some(format!("expected a table for `{context}`")),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(lua: &Lua, src: &str) -> Value {
        lua.load(src).eval().unwrap()
    }

    #[test]
    fn config_entry_deserializes() {
        let lua = Lua::new();
        let v = eval(
            &lua,
            r#"{
                locate = {
                    { filters = { { __kind = "env_file", path = "api/.env", variable = "PORT" } } },
                    { filters = {
                        { __kind = "file", glob = "client/**/*.json" },
                        { __kind = "regex", pattern = "8001" }
                    } },
                },
                override = { __kind = "random_port" },
            }"#,
        );
        let entry = ConfigEntry::from_lua(v, &lua).unwrap();
        assert_eq!(entry.locate.len(), 2);
        assert_eq!(entry.locate[0].filters.len(), 1);
        assert_eq!(entry.locate[1].filters.len(), 2);
        assert!(matches!(entry.overrider, Overrider::RandomPort));
    }

    #[test]
    fn cfg_from_lua_deserializes() {
        let lua = Lua::new();
        let v = eval(
            &lua,
            r#"{
                ["api-port"] = {
                    locate = {
                        { filters = { { __kind = "file", glob = "api/.env" }, { __kind = "regex", pattern = "8001" } } },
                    },
                    override = { __kind = "random_port" },
                },
            }"#,
        );
        let cfg = cfg_from_lua(v, &lua).unwrap();
        assert!(cfg.contains_key("api-port"));
        assert_eq!(cfg["api-port"].locate.len(), 1);
    }

    #[test]
    fn cfg_from_lua_errors_on_non_table() {
        let lua = Lua::new();
        let v = eval(&lua, "42");
        assert!(cfg_from_lua(v, &lua).is_err());
    }
}
