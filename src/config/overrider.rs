use std::path::Path;

use mlua::{FromLua, Lua, Result as LuaResult, Value};

use super::expect_table;
use super::locator::{Target, split_selector};

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
                let port = random_port()?;
                for t in targets {
                    match t {
                        Target::Line {
                            path,
                            line_number,
                            target,
                        } => {
                            replace_in_file(path, *line_number, target, &port.to_string())?;
                        }
                        Target::Json { path, selector } => {
                            replace_json_value(path, selector, serde_json::json!(port))?;
                        }
                    }
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

fn replace_json_value(
    path: &Path,
    selector: &str,
    new_value: serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let mut json: serde_json::Value = serde_json::from_str(&content)?;
    let keys = split_selector(selector)?;
    let (parents, last) = keys.split_at(keys.len() - 1);
    let mut cur = &mut json;
    for k in parents {
        cur = cur
            .get_mut(k.as_str())
            .ok_or_else(|| format!("key `{k}` not found"))?;
    }
    cur[last[0].as_str()] = new_value;
    std::fs::write(path, serde_json::to_string_pretty(&json)? + "\n")?;
    Ok(())
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

        let targets = vec![Target::Line {
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
    fn random_port_apply_rewrites_json_file() {
        let dir = std::env::temp_dir().join("emux_test_overrider_json");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "{\n  \"server\": {\n    \"port\": 8001\n  }\n}\n").unwrap();

        let targets = vec![Target::Json {
            path: path.clone(),
            selector: ".server.port".to_owned(),
        }];
        Overrider::RandomPort.apply(&targets).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        let new_val = json["server"]["port"].as_u64().unwrap();
        assert_ne!(new_val, 8001);
        assert!(new_val > 0 && new_val <= 65535);
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
