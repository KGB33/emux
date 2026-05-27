use std::path::{Path, PathBuf};

use glob::glob;
use grep::matcher::Matcher;
use grep::regex::RegexMatcher;
use grep::searcher::{Searcher, SearcherBuilder, Sink, SinkMatch};
use mlua::{FromLua, Lua, Result as LuaResult, Value};

use super::expect_table;

type ApplyFn = Box<dyn Fn(&str) -> Result<(), Box<dyn std::error::Error>>>;

/// A pipeline of filters that narrows scope from the whole repo to specific locations.
#[derive(Debug)]
pub struct Locator {
    pub filters: Vec<Filter>,
}

/// A located target bundling write metadata with a closure that applies a new value.
pub struct Applicator {
    pub path: PathBuf,
    pub line_number: u64,
    pub old_value: String,
    pub old_line: String,
    apply: ApplyFn,
}

impl std::fmt::Debug for Applicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Applicator")
            .field("path", &self.path)
            .field("line_number", &self.line_number)
            .field("old_value", &self.old_value)
            .field("old_line", &self.old_line)
            .field("apply", &"<fn>")
            .finish()
    }
}

impl Applicator {
    pub fn new(
        path: PathBuf,
        line_number: u64,
        old_value: String,
        old_line: String,
        apply: ApplyFn,
    ) -> Self {
        Self {
            path,
            line_number,
            old_value,
            old_line,
            apply,
        }
    }

    pub fn call(&self, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        (self.apply)(value)
    }
}

/// A single step in a locator pipeline.
#[derive(Debug)]
pub enum Filter {
    /// `files("glob")` — repo → files matching the glob.
    File { glob: String },
    /// `regex("pattern")` — files → line locations matching the pattern.
    Regex { pattern: String },
    /// `envFile("path", "VAR")` — targets a specific variable in a dotenv-style file.
    EnvFile { path: PathBuf, variable: String },
    /// `jsonFile("path", ".key")` or `jsonFile("path", ".parent.child")` — targets a value in a JSON file.
    JsonFile { path: PathBuf, selector: String },
}

impl Locator {
    pub fn locate(&self, dir: &Path) -> Result<Vec<Applicator>, Box<dyn std::error::Error>> {
        let mut paths: Vec<PathBuf> = vec![];
        for filter in &self.filters {
            match filter {
                Filter::File { glob: pattern } => {
                    let full = dir.join(pattern).to_string_lossy().into_owned();
                    paths = glob(&full)?.filter_map(|e| e.ok()).collect();
                }
                Filter::Regex { pattern } => return search_regex(pattern, &paths),
                Filter::EnvFile { path, variable } => {
                    let abs = if path.is_absolute() {
                        path.clone()
                    } else {
                        dir.join(path)
                    };
                    return search_env_file(&abs, variable);
                }
                Filter::JsonFile { path, selector } => {
                    let abs = if path.is_absolute() {
                        path.clone()
                    } else {
                        dir.join(path)
                    };
                    return search_json_file(&abs, selector);
                }
            }
        }
        Ok(vec![])
    }
}

fn search_regex(
    pattern: &str,
    paths: &[PathBuf],
) -> Result<Vec<Applicator>, Box<dyn std::error::Error>> {
    struct MatchSink<'a> {
        path: &'a Path,
        matcher: &'a RegexMatcher,
        matches: Vec<Applicator>,
    }

    impl Sink for MatchSink<'_> {
        type Error = std::io::Error;
        fn matched(&mut self, _: &Searcher, m: &SinkMatch) -> Result<bool, Self::Error> {
            let line_bytes = m.bytes();
            if let Ok(Some(mat)) = self.matcher.find(line_bytes) {
                let old_value = std::str::from_utf8(&line_bytes[mat.start()..mat.end()])
                    .unwrap_or("")
                    .to_owned();
                let old_line = std::str::from_utf8(line_bytes)
                    .unwrap_or("")
                    .trim_end_matches(['\n', '\r'])
                    .to_owned();
                let path = self.path.to_owned();
                let line_number = m.line_number().unwrap_or(0);
                let old_value_clone = old_value.clone();
                self.matches.push(Applicator::new(
                    path.clone(),
                    line_number,
                    old_value,
                    old_line,
                    Box::new(move |new_val| {
                        replace_in_file(&path, line_number, &old_value_clone, new_val)
                            .map_err(Into::into)
                    }),
                ));
            }
            Ok(true)
        }
    }

    let matcher = RegexMatcher::new(pattern)?;
    let mut searcher = SearcherBuilder::new().line_number(true).build();
    let mut all = vec![];
    for path in paths {
        let mut sink = MatchSink {
            path,
            matcher: &matcher,
            matches: vec![],
        };
        searcher.search_path(&matcher, path, &mut sink)?;
        all.extend(sink.matches);
    }
    Ok(all)
}

fn search_env_file(
    path: &Path,
    variable: &str,
) -> Result<Vec<Applicator>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let prefix = format!("{variable}=");
    let applicators = content
        .lines()
        .enumerate()
        .filter(|(_, line)| line.starts_with(&prefix))
        .map(|(i, line)| {
            let old_value = line[prefix.len()..].to_owned();
            let old_line = line.to_owned();
            let line_number = (i + 1) as u64;
            let path_clone = path.to_owned();
            let old_val_clone = old_value.clone();
            Applicator::new(
                path.to_owned(),
                line_number,
                old_value,
                old_line,
                Box::new(move |new_val| {
                    replace_in_file(&path_clone, line_number, &old_val_clone, new_val)
                        .map_err(Into::into)
                }),
            )
        })
        .collect();
    Ok(applicators)
}

fn search_json_file(
    path: &Path,
    selector: &str,
) -> Result<Vec<Applicator>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;
    let keys = split_selector(selector)?;
    let mut v = &json;
    for k in &keys {
        v = v
            .get(k.as_str())
            .ok_or_else(|| format!("key `{k}` not found in `{selector}`"))?;
    }
    let old_value = v.to_string();
    let last_key = keys.last().unwrap();
    let search = format!("\"{}\":", last_key);
    let (line_number, old_line) = content
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(&search) && l.contains(old_value.as_str()))
        .map(|(i, l)| ((i + 1) as u64, l.to_owned()))
        .unwrap_or((0, String::new()));

    let path_clone = path.to_owned();
    let selector_clone = selector.to_owned();
    Ok(vec![Applicator::new(
        path.to_owned(),
        line_number,
        old_value,
        old_line,
        Box::new(move |new_val| {
            // Parse new_val as JSON (e.g. "54321" → Number) so native JSON types are preserved.
            let json_new: serde_json::Value = serde_json::from_str(new_val)
                .unwrap_or_else(|_| serde_json::Value::String(new_val.to_owned()));
            replace_json_value(&path_clone, &selector_clone, json_new)
        }),
    )])
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

pub(super) fn split_selector(selector: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let keys: Vec<String> = selector
        .trim_start_matches('.')
        .split('.')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    if keys.is_empty() {
        return Err("selector must contain at least one key".into());
    }
    Ok(keys)
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
            "json_file" => Ok(Filter::JsonFile {
                path: PathBuf::from(table.get::<String>("path")?),
                selector: table.get("selector")?,
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
    fn locate_regex_finds_matching_lines() {
        let dir = std::env::temp_dir().join("emux_test_search");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "no match here\nPORT=8001\nalso no match\n").unwrap();

        let locator = Locator {
            filters: vec![
                Filter::File {
                    glob: "config.json".to_string(),
                },
                Filter::Regex {
                    pattern: "PORT=8001".to_string(),
                },
            ],
        };
        let applicators = locator.locate(&dir).unwrap();
        assert_eq!(applicators.len(), 1);
        assert_eq!(applicators[0].line_number, 2);
        assert_eq!(applicators[0].old_value, "PORT=8001");
    }

    #[test]
    fn locate_file_glob_limits_search_scope() {
        let dir = std::env::temp_dir().join("emux_test_expand");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.json"), "PORT=8001\n").unwrap();
        std::fs::write(dir.join("other.txt"), "PORT=8001\n").unwrap();

        let locator = Locator {
            filters: vec![
                Filter::File {
                    glob: "*.json".to_string(),
                },
                Filter::Regex {
                    pattern: "PORT=8001".to_string(),
                },
            ],
        };
        let applicators = locator.locate(&dir).unwrap();
        assert_eq!(applicators.len(), 1);
        assert_eq!(applicators[0].path, dir.join("config.json"));
    }

    #[test]
    fn locate_env_file_finds_variable() {
        let dir = std::env::temp_dir().join("emux_test_envfile");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env");
        std::fs::write(&path, "HOST=localhost\nPORT=8001\nDEBUG=true\n").unwrap();

        let locator = Locator {
            filters: vec![Filter::EnvFile {
                path: path.clone(),
                variable: "PORT".to_string(),
            }],
        };
        let applicators = locator.locate(&dir).unwrap();
        assert_eq!(applicators.len(), 1);
        assert_eq!(applicators[0].line_number, 2);
        assert_eq!(applicators[0].old_value, "8001");
    }

    #[test]
    fn locate_env_file_applicator_rewrites_file() {
        let dir = std::env::temp_dir().join("emux_test_envfile_apply");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env");
        std::fs::write(&path, "HOST=localhost\nPORT=8001\nDEBUG=true\n").unwrap();

        let locator = Locator {
            filters: vec![Filter::EnvFile {
                path: path.clone(),
                variable: "PORT".to_string(),
            }],
        };
        let applicators = locator.locate(&dir).unwrap();
        applicators[0].call("9999").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("PORT=9999"), "PORT should be rewritten");
    }

    #[test]
    fn locate_json_file_returns_applicator() {
        let dir = std::env::temp_dir().join("emux_test_jsonfile_flat");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(
            &path,
            "{\n  \"host\": \"localhost\",\n  \"port\": 8001\n}\n",
        )
        .unwrap();

        let locator = Locator {
            filters: vec![Filter::JsonFile {
                path: path.clone(),
                selector: ".port".to_string(),
            }],
        };
        let applicators = locator.locate(&dir).unwrap();
        assert_eq!(applicators.len(), 1);
        assert_eq!(applicators[0].old_value, "8001");
    }

    #[test]
    fn locate_json_file_applicator_rewrites_file() {
        let dir = std::env::temp_dir().join("emux_test_jsonfile_apply");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "{\n  \"port\": 8001\n}\n").unwrap();

        let locator = Locator {
            filters: vec![Filter::JsonFile {
                path: path.clone(),
                selector: ".port".to_string(),
            }],
        };
        let applicators = locator.locate(&dir).unwrap();
        applicators[0].call("9999").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(json["port"].as_u64().unwrap(), 9999);
    }

    #[test]
    fn locate_json_file_validates_nested_selector() {
        let dir = std::env::temp_dir().join("emux_test_jsonfile_nested");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "{\n  \"server\": {\n    \"port\": 9000\n  }\n}\n").unwrap();

        let locator = Locator {
            filters: vec![Filter::JsonFile {
                path: path.clone(),
                selector: ".server.port".to_string(),
            }],
        };
        let applicators = locator.locate(&dir).unwrap();
        assert_eq!(applicators.len(), 1);
        assert_eq!(applicators[0].old_value, "9000");
    }

    #[test]
    fn locate_json_file_errors_on_missing_key() {
        let dir = std::env::temp_dir().join("emux_test_jsonfile_missing");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "{\"port\": 8001}\n").unwrap();

        let locator = Locator {
            filters: vec![Filter::JsonFile {
                path: path.clone(),
                selector: ".missing".to_string(),
            }],
        };
        assert!(locator.locate(&dir).is_err());
    }

    #[test]
    fn replace_in_file_preserves_crlf_endings() {
        let dir = std::env::temp_dir().join("emux_test_locator_crlf");
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
        let dir = std::env::temp_dir().join("emux_test_locator_trailing");
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
    fn filter_json_file_deserializes() {
        let lua = Lua::new();
        let v = eval(
            &lua,
            r#"{ __kind = "json_file", path = "config.json", selector = ".server.port" }"#,
        );
        let f = Filter::from_lua(v, &lua).unwrap();
        assert!(
            matches!(f, Filter::JsonFile { path, selector } if *path == *"config.json" && selector == ".server.port")
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
