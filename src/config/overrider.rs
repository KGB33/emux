use std::path::Path;

use mlua::{FromLua, Lua, Result as LuaResult, Value};

use super::expect_table;
use super::locator::Target;

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

    pub fn apply(&self, targets: &[Target]) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Overrider::RandomPort => {
                let port = random_port()?.to_string();
                // Each target must have a unique (path, line_number) pair. replace_in_file
                // reads the file fresh each iteration; two targets sharing the same line
                // would cause the second replacement to silently no-op.
                for t in targets {
                    replace_in_file(&t.path, t.line_number, &t.target, &port)?;
                }
                Ok(())
            }
        }
    }
}

fn random_port() -> Result<u16, std::io::Error> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
}

fn replace_in_file(
    path: &Path,
    line_number: u64,
    old: &str,
    new: &str,
) -> Result<(), std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    let result: String = content
        .split_inclusive('\n')
        .enumerate()
        .map(|(i, line)| {
            if (i + 1) as u64 != line_number {
                return line.to_owned();
            }
            let ending = if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            let body = &line[..line.len() - ending.len()];
            format!("{}{ending}", body.replacen(old, new, 1))
        })
        .collect();
    std::fs::write(path, result)
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
    fn random_port_apply_rewrites_file() {
        let dir = std::env::temp_dir().join("emux_test_overrider");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env");
        std::fs::write(&path, "HOST=localhost\nPORT=8001\nDEBUG=true\n").unwrap();

        let targets = vec![Target {
            path: path.clone(),
            line_number: 2,
            target: "8001".to_owned(),
        }];
        Overrider::RandomPort.apply(&targets).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let port_line = content.lines().nth(1).unwrap();
        assert!(port_line.starts_with("PORT="));
        let new_val: u16 = port_line["PORT=".len()..].parse().unwrap();
        assert_ne!(new_val, 8001);
    }

    #[test]
    fn replace_in_file_preserves_crlf_endings() {
        let dir = std::env::temp_dir().join("emux_test_overrider_crlf");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env_crlf");
        std::fs::write(&path, "HOST=localhost\r\nPORT=8001\r\nDEBUG=true\r\n").unwrap();

        replace_in_file(&path, 2, "8001", "9999").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            content.contains("PORT=9999\r\n"),
            "CRLF ending must be preserved"
        );
        assert_eq!(
            content.matches("\r\n").count(),
            3,
            "all three CRLF endings must survive"
        );
    }

    #[test]
    fn replace_in_file_preserves_multiple_trailing_newlines() {
        let dir = std::env::temp_dir().join("emux_test_overrider_trailing");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env_trailing");
        std::fs::write(&path, "HOST=localhost\nPORT=8001\nDEBUG=true\n\n").unwrap();

        replace_in_file(&path, 2, "8001", "9999").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            content.ends_with("\n\n"),
            "double trailing newline must be preserved"
        );
        assert!(content.contains("PORT=9999"));
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
