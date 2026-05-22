use std::path::PathBuf;

use mlua::{FromLua, Lua, Result as LuaResult, Value};

use super::expect_table;

/// A pipeline of filters that narrows scope from the whole repo to specific locations.
#[derive(Debug)]
pub struct Locator {
    pub filters: Vec<Filter>,
}

/// A single step in a locator pipeline.
#[derive(Debug)]
pub enum Filter {
    /// `file("glob")` / `files("glob")` — repo → files matching the glob.
    File { glob: String },
    /// `regex("pattern")` — files → line locations matching the pattern.
    Regex { pattern: String },
    /// `env-file("path", "VAR")` — targets a specific variable in a dotenv-style file.
    EnvFile { path: PathBuf, variable: String },
}

impl FromLua for Locator {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        let table = expect_table(value, "Locator")?;
        let filters_table = expect_table(table.get("filters")?, "filters")?;
        let len = filters_table.raw_len();
        let mut filters = Vec::with_capacity(len as usize);
        for i in 1..=len {
            filters.push(Filter::from_lua(filters_table.raw_get(i)?, lua)?);
        }
        Ok(Locator { filters })
    }
}

impl FromLua for Filter {
    fn from_lua(value: Value, _lua: &Lua) -> LuaResult<Self> {
        let table = expect_table(value, "Filter")?;
        let kind: String = table.get("__kind")?;
        match kind.as_str() {
            "file" => Ok(Filter::File {
                glob: table.get("glob")?,
            }),
            "regex" => Ok(Filter::Regex {
                pattern: table.get("pattern")?,
            }),
            "env_file" => Ok(Filter::EnvFile {
                path: PathBuf::from(table.get::<String>("path")?),
                variable: table.get("variable")?,
            }),
            other => Err(mlua::Error::FromLuaConversionError {
                from: "table",
                to: "Filter".to_string(),
                message: Some(format!("unknown filter kind `{other}`")),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(lua: &Lua, src: &str) -> Value {
        lua.load(src).eval().unwrap()
    }

    #[test]
    fn filter_file_deserializes() {
        let lua = Lua::new();
        let v = eval(&lua, r#"{ __kind = "file", glob = "src/**/*.rs" }"#);
        let f = Filter::from_lua(v, &lua).unwrap();
        assert!(matches!(f, Filter::File { glob } if glob == "src/**/*.rs"));
    }

    #[test]
    fn filter_regex_deserializes() {
        let lua = Lua::new();
        let v = eval(&lua, r#"{ __kind = "regex", pattern = "8001" }"#);
        let f = Filter::from_lua(v, &lua).unwrap();
        assert!(matches!(f, Filter::Regex { pattern } if pattern == "8001"));
    }

    #[test]
    fn filter_env_file_deserializes() {
        let lua = Lua::new();
        let v = eval(&lua, r#"{ __kind = "env_file", path = "api/.env", variable = "PORT" }"#);
        let f = Filter::from_lua(v, &lua).unwrap();
        assert!(
            matches!(f, Filter::EnvFile { path, variable } if path == PathBuf::from("api/.env") && variable == "PORT")
        );
    }

    #[test]
    fn filter_unknown_kind_errors() {
        let lua = Lua::new();
        let v = eval(&lua, r#"{ __kind = "unknown" }"#);
        assert!(Filter::from_lua(v, &lua).is_err());
    }

    #[test]
    fn locator_single_filter() {
        let lua = Lua::new();
        let v = eval(&lua, r#"{ filters = { { __kind = "file", glob = "*.lua" } } }"#);
        let loc = Locator::from_lua(v, &lua).unwrap();
        assert_eq!(loc.filters.len(), 1);
        assert!(matches!(&loc.filters[0], Filter::File { glob } if glob == "*.lua"));
    }

    #[test]
    fn locator_chained_filters() {
        let lua = Lua::new();
        let v = eval(
            &lua,
            r#"{ filters = {
                { __kind = "file", glob = "client/**/*.json" },
                { __kind = "regex", pattern = "8001" }
            } }"#,
        );
        let loc = Locator::from_lua(v, &lua).unwrap();
        assert_eq!(loc.filters.len(), 2);
        assert!(matches!(&loc.filters[0], Filter::File { .. }));
        assert!(matches!(&loc.filters[1], Filter::Regex { pattern } if pattern == "8001"));
    }
}
