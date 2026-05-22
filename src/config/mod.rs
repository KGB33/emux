mod locator;
mod overrider;

pub use locator::Locator;
use locator::Target;
pub use overrider::Overrider;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use mlua::{FromLua, Lua, Result as LuaResult, Table, Value};

pub type Cfg = HashMap<String, ConfigEntry>;

#[derive(Debug)]
pub struct ConfigEntry {
    pub locate: Vec<Locator>,
    pub overrider: Overrider, // `override` is a reserved keyword
}

pub struct DiffEntry {
    pub entry_name: String,
    pub path: PathBuf,
    pub line_number: u64,
    pub old_line: String,
    pub new_line: String,
    pub old_value: String,
    pub new_value: String,
}

pub fn load_config_file(file: &Path) -> Result<Cfg, Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(file)?;
    let name = file.display().to_string();
    let lua = Lua::new();
    crate::lua_api::load(&lua)?;
    let cfg_val: Value = match file.extension().and_then(|e| e.to_str()) {
        Some("lua") => lua.load(&source).set_name(&name).eval()?,
        Some("fnl") => {
            let lua_src = crate::lua_api::compile_fennel(&lua, &source, &name)?;
            lua.load(&lua_src).set_name(&name).eval()?
        }
        _ => return Err(format!("unsupported file type `{}`", file.display()).into()),
    };
    cfg_from_lua(cfg_val, &lua).map_err(Into::into)
}

pub fn diff_cfg(cfg: &Cfg, dir: &Path) -> Result<Vec<DiffEntry>, Box<dyn std::error::Error>> {
    let mut out = vec![];
    for (name, entry) in cfg {
        let targets = entry.locate_all(dir)?;
        let ir = entry.overrider.ir_label();
        let mut file_cache: HashMap<PathBuf, String> = HashMap::new();

        for target in &targets {
            let content = match file_cache.entry(target.path.clone()) {
                std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(std::fs::read_to_string(&target.path)?)
                }
            };
            let old_line = content
                .lines()
                .nth((target.line_number - 1) as usize)
                .unwrap_or("")
                .to_owned();
            let new_line = old_line.replacen(&target.target, ir, 1);
            out.push(DiffEntry {
                entry_name: name.clone(),
                path: target.path.clone(),
                line_number: target.line_number,
                old_value: target.target.clone(),
                new_value: ir.to_owned(),
                old_line,
                new_line,
            });
        }
    }
    Ok(out)
}

pub fn apply_cfg(cfg: &Cfg, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in cfg.values() {
        entry.overrider.apply(&entry.locate_all(dir)?)?;
    }
    Ok(())
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

impl ConfigEntry {
    fn locate_all(&self, dir: &Path) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
        let mut targets = vec![];
        for locator in &self.locate {
            targets.extend(locator.locate(dir)?);
        }
        Ok(targets)
    }
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

    #[test]
    fn load_config_file_lua() {
        let dir = std::env::temp_dir().join("emux_test_load_cfg_lua");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("emux.lua");
        std::fs::write(
            &path,
            r#"
            local cfg = {
                ["api-port"] = {
                    locate = { emux.l.envFile("api/.env", "PORT") },
                    override = emux.o.randPort,
                },
            }
            return cfg
            "#,
        )
        .unwrap();
        let cfg = load_config_file(&path).unwrap();
        assert!(cfg.contains_key("api-port"));
    }

    #[test]
    fn load_config_file_fennel() {
        let dir = std::env::temp_dir().join("emux_test_load_cfg_fnl");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("emux.fnl");
        std::fs::write(
            &path,
            r#"
            (local cfg
              {"api-port"
               {:locate [(emux.l.envFile "api/.env" "PORT")]
                :override emux.o.randPort}})
            cfg
            "#,
        )
        .unwrap();
        let cfg = load_config_file(&path).unwrap();
        assert!(cfg.contains_key("api-port"));
    }

    #[test]
    fn load_config_file_unsupported_ext_errors() {
        let dir = std::env::temp_dir().join("emux_test_load_cfg_ext");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("emux.txt");
        std::fs::write(&path, "").unwrap();
        assert!(load_config_file(&path).is_err());
    }
}
