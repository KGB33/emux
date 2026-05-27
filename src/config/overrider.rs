use mlua::{FromLua, Lua, Result as LuaResult, Value};

use super::expect_table;
use super::locator::Applicator;

#[derive(Debug)]
pub enum Overrider {
    /// `int.random` / `random-port` — generates a random port number.
    RandomPort,
}

impl Overrider {
    pub fn ir_label(&self) -> &'static str {
        match self {
            Overrider::RandomPort => "<random_port>",
        }
    }

    pub fn generate(&self) -> Result<String, Box<dyn std::error::Error>> {
        match self {
            Overrider::RandomPort => Ok(random_port()?.to_string()),
        }
    }

    pub fn apply(&self, applicators: &[Applicator]) -> Result<(), Box<dyn std::error::Error>> {
        let value = self.generate()?;
        for a in applicators {
            a.call(&value)?;
        }
        Ok(())
    }
}

fn random_port() -> Result<u16, std::io::Error> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
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
    fn random_port_generates_valid_port() {
        let port_str = Overrider::RandomPort.generate().unwrap();
        let port: u16 = port_str.parse().unwrap();
        assert!(port > 0);
    }

    #[test]
    fn random_port_deserializes() {
        let lua = Lua::new();
        let v = lua.load(r#"{ __kind = "random_port" }"#).eval().unwrap();
        assert!(matches!(
            Overrider::from_lua(v, &lua).unwrap(),
            Overrider::RandomPort
        ));
    }

    #[test]
    fn unknown_kind_errors() {
        let lua = Lua::new();
        let v = lua.load(r#"{ __kind = "unknown" }"#).eval().unwrap();
        assert!(Overrider::from_lua(v, &lua).is_err());
    }
}
