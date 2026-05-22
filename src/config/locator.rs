use std::path::{Path, PathBuf};

use glob::glob;
use grep::regex::RegexMatcher;
use grep::searcher::{Searcher, SearcherBuilder, Sink, SinkMatch};
use mlua::{FromLua, Lua, Result as LuaResult, Value};

use super::expect_table;

/// A pipeline of filters that narrows scope from the whole repo to specific locations.
#[derive(Debug)]
pub struct Locator {
    pub filters: Vec<Filter>,
}

/// A located line — the exact substring an overrider should find and replace.
#[derive(Debug)]
pub struct Target {
    pub path: PathBuf,
    pub line_number: u64,
    pub target: String,
}

struct MatchSink<'p> {
    path: &'p Path,
    pattern: &'p str,
    matches: Vec<Target>,
}

impl Sink for MatchSink<'_> {
    type Error = std::io::Error;
    fn matched(&mut self, _: &Searcher, m: &SinkMatch) -> Result<bool, Self::Error> {
        self.matches.push(Target {
            path: self.path.to_owned(),
            line_number: m.line_number().unwrap_or(0),
            target: self.pattern.to_owned(),
        });
        Ok(true)
    }
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

impl Filter {
    pub fn expand(&self, dir: &Path) -> Result<Vec<PathBuf>, glob::PatternError> {
        match self {
            Filter::File { glob: pattern } => {
                let full = dir.join(pattern).to_string_lossy().into_owned();
                Ok(glob(&full)?.filter_map(|e| e.ok()).collect())
            }
            _ => Ok(vec![]),
        }
    }

    pub fn search(&self, paths: &[PathBuf]) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
        match self {
            Filter::Regex { pattern } => {
                let matcher = RegexMatcher::new(pattern)?;
                let mut searcher = SearcherBuilder::new().line_number(true).build();
                let mut all = vec![];
                for path in paths {
                    let mut sink = MatchSink { path, pattern, matches: vec![] };
                    searcher.search_path(&matcher, path, &mut sink)?;
                    all.extend(sink.matches);
                }
                Ok(all)
            }
            Filter::EnvFile { path, variable } => {
                let content = std::fs::read_to_string(path)?;
                let prefix = format!("{variable}=");
                let targets = content
                    .lines()
                    .enumerate()
                    .filter(|(_, line)| line.starts_with(&prefix))
                    .map(|(i, line)| Target {
                        path: path.clone(),
                        line_number: (i + 1) as u64,
                        target: line[prefix.len()..].to_owned(),
                    })
                    .collect();
                Ok(targets)
            }
            _ => Ok(vec![]),
        }
    }
}

impl Locator {
    pub fn locate(&self, dir: &Path) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
        let mut paths: Vec<PathBuf> = vec![];
        for filter in &self.filters {
            match filter {
                Filter::File { .. } => paths = filter.expand(dir)?,
                Filter::Regex { .. } => return filter.search(&paths),
                Filter::EnvFile { path, variable } => {
                    let abs = if path.is_absolute() { path.clone() } else { dir.join(path) };
                    return Filter::EnvFile { path: abs, variable: variable.clone() }.search(&[]);
                }
            }
        }
        Ok(vec![])
    }
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
    fn filter_regex_search_finds_matching_lines() {
        let dir = std::env::temp_dir().join("emux_test_search");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "no match here\nPORT=8001\nalso no match\n").unwrap();

        let filter = Filter::Regex { pattern: "PORT=8001".to_string() };
        let targets = filter.search(&[path]).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].line_number, 2);
        assert_eq!(targets[0].target, "PORT=8001");
    }

    #[test]
    fn filter_file_expand_finds_matching_files() {
        let dir = std::env::temp_dir().join("emux_test_expand");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.json"), "{}").unwrap();
        std::fs::write(dir.join("other.txt"), "").unwrap();

        let filter = Filter::File {
            glob: "*.json".to_string(),
        };
        let found = filter.expand(&dir).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], dir.join("config.json"));
    }

    #[test]
    fn filter_env_file_search_finds_variable() {
        let dir = std::env::temp_dir().join("emux_test_envfile");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env");
        std::fs::write(&path, "HOST=localhost\nPORT=8001\nDEBUG=true\n").unwrap();

        let filter = Filter::EnvFile { path: path.clone(), variable: "PORT".to_string() };
        let targets = filter.search(&[]).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].line_number, 2);
        assert_eq!(targets[0].target, "8001");
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
        let v = eval(
            &lua,
            r#"{ __kind = "env_file", path = "api/.env", variable = "PORT" }"#,
        );
        let f = Filter::from_lua(v, &lua).unwrap();
        assert!(
            matches!(f, Filter::EnvFile { path, variable } if *path == *"api/.env" && variable == "PORT")
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
        let v = eval(
            &lua,
            r#"{ filters = { { __kind = "file", glob = "*.lua" } } }"#,
        );
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
