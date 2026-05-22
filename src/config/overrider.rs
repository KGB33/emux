use mlua::{FromLua, Lua, Result as LuaResult, Value};

use super::expect_table;

#[derive(Debug)]
pub enum Overrider {
    /// `int.random` / `random-port` — generates a random port number.
    RandomPort,
}

impl FromLua for Overrider {
    fn from_lua(value: Value, _lua: &Lua) -> LuaResult<Self> {
        let table = expect_table(value, "Overrider")?;
        let kind: String = table.get("__kind")?;
        match kind.as_str() {
            "random_port" => Ok(Overrider::RandomPort),
            other => Err(mlua::Error::FromLuaConversionError {
                from: "table",
                to: "Overrider".to_string(),
                message: Some(format!("unknown overrider kind `{other}`")),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_port_deserializes() {
        let lua = Lua::new();
        let v = lua.load(r#"{ __kind = "random_port" }"#).eval().unwrap();
        assert!(matches!(Overrider::from_lua(v, &lua).unwrap(), Overrider::RandomPort));
    }

    #[test]
    fn unknown_kind_errors() {
        let lua = Lua::new();
        let v = lua.load(r#"{ __kind = "unknown" }"#).eval().unwrap();
        assert!(Overrider::from_lua(v, &lua).is_err());
    }
}
