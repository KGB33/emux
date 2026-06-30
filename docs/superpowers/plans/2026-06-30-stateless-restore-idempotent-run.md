# Stateless Restore + Idempotent Run Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `emux run` idempotent and add a stateless `emux restore`, with overrides computed purely from config + worktree.

**Architecture:** Port overrides become a deterministic function of the worktree path + entry name (FNV-1a → ephemeral port), so re-running recomputes the same value. Each config entry declares a `base` value that `restore` writes back. The value-based `regex`/`files` locators are deleted, leaving only key-anchored `envFile`/`jsonFile` locators that resolve their position regardless of the file's current contents.

**Tech Stack:** Rust 2024 edition, `clap`, `mlua` (Lua 5.4 + vendored Fennel), `serde_json`. Removing `glob` and `grep`.

## Global Constraints

- **No sidecar state files.** Everything needed to apply or undo must come from the config file + worktree.
- **Deterministic port range:** `49152..=65535` (ephemeral), via locally-implemented FNV-1a (64-bit) — never `std::hash::DefaultHasher`.
- **`base` is read as a string;** a Lua number coerces to its string form. `base` is optional at parse time, required by `restore`, never read by `run`.
- Each task must end with `cargo test` green and a commit.
- Follow the existing pattern: unit/integration tests live in `#[cfg(test)] mod tests` blocks inside the file under test, using `std::env::temp_dir()` fixtures.

---

### Task 1: Delete the `regex` and `files` locators

**Files:**
- Modify: `src/config/locator.rs`
- Modify: `src/config/mod.rs` (two tests use `file`/`regex` filters)
- Modify: `src/emux.fnl`
- Modify: `src/lua_api.rs` (tests)
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: nothing new.
- Produces: `Filter` enum with exactly two variants — `Filter::EnvFile { path: PathBuf, variable: String }` and `Filter::JsonFile { path: PathBuf, selector: String }`. `Locator::locate(&self, dir: &Path) -> Result<Vec<Applicator>, Box<dyn std::error::Error>>` unchanged in signature.

- [ ] **Step 1: Remove the `File`/`Regex` variants and glob/grep code from `locator.rs`**

In `src/config/locator.rs`:

Delete the top imports for glob and grep:
```rust
use glob::glob;
use grep::matcher::Matcher;
use grep::regex::RegexMatcher;
use grep::searcher::{Searcher, SearcherBuilder, Sink, SinkMatch};
```
(Keep `use std::path::{Path, PathBuf};`, the `mlua` import, and `use super::expect_table;`.)

Replace the `Filter` enum with:
```rust
/// A single step in a locator pipeline.
#[derive(Debug)]
pub enum Filter {
    /// `envFile("path", "VAR")` — targets a specific variable in a dotenv-style file.
    EnvFile { path: PathBuf, variable: String },
    /// `jsonFile("path", ".key")` or `jsonFile("path", ".parent.child")` — targets a value in a JSON file.
    JsonFile { path: PathBuf, selector: String },
}
```

Replace `Locator::locate` with:
```rust
impl Locator {
    pub fn locate(&self, dir: &Path) -> Result<Vec<Applicator>, Box<dyn std::error::Error>> {
        for filter in &self.filters {
            match filter {
                Filter::EnvFile { path, variable } => {
                    let abs = if path.is_absolute() { path.clone() } else { dir.join(path) };
                    return search_env_file(&abs, variable);
                }
                Filter::JsonFile { path, selector } => {
                    let abs = if path.is_absolute() { path.clone() } else { dir.join(path) };
                    return search_json_file(&abs, selector);
                }
            }
        }
        Ok(vec![])
    }
}
```

Delete the entire `fn search_regex(...)` function.

In `impl FromLua for Filter`, delete the `"file"` and `"regex"` match arms, leaving:
```rust
        match kind.as_str() {
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
```

- [ ] **Step 2: Remove the regex/files tests in `locator.rs`**

Delete these `#[test]` functions from the `tests` module in `src/config/locator.rs`:
`locate_regex_finds_matching_lines`, `locate_file_glob_limits_search_scope`,
`filter_file_deserializes`, `filter_regex_deserializes`,
`locator_single_filter`, `locator_chained_filters`.

Add a replacement locator test that uses a key-anchored filter:
```rust
    #[test]
    fn locator_single_env_filter() {
        let lua = Lua::new();
        let v = eval(
            &lua,
            r#"{ filters = { { __kind = "env_file", path = "api/.env", variable = "PORT" } } }"#,
        );
        let loc = Locator::from_lua(v, &lua).unwrap();
        assert_eq!(loc.filters.len(), 1);
        assert!(matches!(&loc.filters[0], Filter::EnvFile { variable, .. } if variable == "PORT"));
    }
```

- [ ] **Step 3: Remove `files`/`regex` from `src/config/mod.rs` tests**

In `src/config/mod.rs`, `config_entry_deserializes`: replace the second locate entry (the `file` + `regex` filters) with a `json_file` filter and update the assertion. The test body becomes:
```rust
        let v = eval(
            &lua,
            r#"{
                locate = {
                    { filters = { { __kind = "env_file", path = "api/.env", variable = "PORT" } } },
                    { filters = { { __kind = "json_file", path = "client/env.json", selector = ".port" } } },
                },
                override = { __kind = "random_port" },
            }"#,
        );
        let entry = ConfigEntry::from_lua(v, &lua).unwrap();
        assert_eq!(entry.locate.len(), 2);
        assert_eq!(entry.locate[0].filters.len(), 1);
        assert_eq!(entry.locate[1].filters.len(), 1);
        assert!(matches!(entry.overrider, Overrider::RandomPort));
```

In `cfg_from_lua_deserializes`: replace the `file` + `regex` filters with a single `env_file` filter:
```rust
        let v = eval(
            &lua,
            r#"{
                ["api-port"] = {
                    locate = {
                        { filters = { { __kind = "env_file", path = "api/.env", variable = "PORT" } } },
                    },
                    override = { __kind = "random_port" },
                },
            }"#,
        );
```
(Leave `override = { __kind = "random_port" }` as-is for now; it is renamed in Task 2.)

- [ ] **Step 4: Remove `files`/`regex` from the Fennel library and its tests**

Replace `src/emux.fnl` with:
```fennel
(local emux
  {:l {:envFile (fn [path variable]
                  {:filters [{:__kind :env_file :path path :variable variable}]})
       :jsonFile (fn [path selector]
                   {:filters [{:__kind :json_file :path path :selector selector}]})}
   :o {:randPort {:__kind :random_port}}})

emux
```
(`randPort` is renamed in Task 2.)

In `src/lua_api.rs`, delete the tests `emux_l_files_returns_locator` and `emux_l_regex_returns_locator_with_both_filters`.

- [ ] **Step 5: Drop the unused dependencies**

In `Cargo.toml`, delete these two lines:
```toml
glob = "0.3.3"
grep = "0.4.1"
```

- [ ] **Step 6: Build and test**

Run: `cargo test`
Expected: PASS, no warnings about unused `glob`/`grep`. If the compiler reports unused imports in `locator.rs`, remove them.

- [ ] **Step 7: Commit**

```bash
git add src/config/locator.rs src/config/mod.rs src/emux.fnl src/lua_api.rs Cargo.toml Cargo.lock
git commit -m "refactor: drop regex/files locators, keep key-anchored locators

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Make the port override deterministic and rename to `port`

**Files:**
- Modify: `src/config/overrider.rs`
- Modify: `src/config/mod.rs` (`apply_cfg`, `diff_cfg`, tests)
- Modify: `src/emux.fnl`
- Modify: `src/lua_api.rs` (test)

**Interfaces:**
- Consumes: `Applicator` (unchanged), `Locator::locate`.
- Produces:
  - `Overrider::Port` (replaces `Overrider::RandomPort`); Lua `__kind` is `"port"`.
  - `Overrider::value(&self, worktree: &Path, entry_name: &str) -> Result<String, Box<dyn std::error::Error>>`
  - `Overrider::apply(&self, worktree: &Path, entry_name: &str, applicators: &[Applicator]) -> Result<(), Box<dyn std::error::Error>>`
  - Private `fn fnv1a_64(bytes: &[u8]) -> u64` and `fn deterministic_port(worktree: &Path, entry_name: &str) -> u16`.

- [ ] **Step 1: Write the failing determinism tests**

In `src/config/overrider.rs`, replace the `tests` module contents with:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_empty_is_offset_basis() {
        assert_eq!(fnv1a_64(b""), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn fnv1a_differs_by_input() {
        assert_ne!(fnv1a_64(b"hello"), fnv1a_64(b"world"));
    }

    #[test]
    fn deterministic_port_is_stable_and_in_range() {
        let dir = std::env::temp_dir();
        let a = deterministic_port(&dir, "api-port");
        let b = deterministic_port(&dir, "api-port");
        assert_eq!(a, b);
        assert!((49152..=65535).contains(&a));
    }

    #[test]
    fn deterministic_port_varies_by_entry() {
        let dir = std::env::temp_dir();
        assert_ne!(
            deterministic_port(&dir, "api-port"),
            deterministic_port(&dir, "db-port")
        );
    }

    #[test]
    fn port_value_parses_into_range() {
        let v = Overrider::Port.value(&std::env::temp_dir(), "x").unwrap();
        let n: u16 = v.parse().unwrap();
        assert!((49152..=65535).contains(&n));
    }

    #[test]
    fn port_apply_zero_applicators_succeeds() {
        Overrider::Port
            .apply(&std::env::temp_dir(), "x", &[])
            .unwrap();
    }

    #[test]
    fn port_deserializes() {
        let lua = Lua::new();
        let v = lua.load(r#"{ __kind = "port" }"#).eval().unwrap();
        assert!(matches!(
            Overrider::from_lua(v, &lua).unwrap(),
            Overrider::Port
        ));
    }

    #[test]
    fn unknown_kind_errors() {
        let lua = Lua::new();
        let v = lua.load(r#"{ __kind = "unknown" }"#).eval().unwrap();
        assert!(Overrider::from_lua(v, &lua).is_err());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib overrider`
Expected: FAIL to compile (`fnv1a_64`, `deterministic_port`, `Overrider::Port` not found).

- [ ] **Step 3: Rewrite the overrider implementation**

In `src/config/overrider.rs`, replace everything above the `tests` module with:
```rust
use std::path::Path;

use mlua::{FromLua, Lua, Result as LuaResult, Value};

use super::expect_table;
use super::locator::Applicator;

#[derive(Debug)]
pub enum Overrider {
    /// `emux.o.port` — a deterministic free-range port, stable per worktree + entry.
    Port,
}

impl Overrider {
    /// Compute the override value for this entry in this worktree.
    pub fn value(
        &self,
        worktree: &Path,
        entry_name: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        match self {
            Overrider::Port => Ok(deterministic_port(worktree, entry_name).to_string()),
        }
    }

    pub fn apply(
        &self,
        worktree: &Path,
        entry_name: &str,
        applicators: &[Applicator],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let value = self.value(worktree, entry_name)?;
        for a in applicators {
            a.apply(&value)?;
        }
        Ok(())
    }
}

/// FNV-1a (64-bit). Implemented locally so the hash is stable across processes
/// and machines — unlike `std::hash::DefaultHasher`, whose keying is unspecified.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Deterministic ephemeral port (49152..=65535) seeded by the absolute worktree
/// path and the entry name.
fn deterministic_port(worktree: &Path, entry_name: &str) -> u16 {
    let abs = std::fs::canonicalize(worktree).unwrap_or_else(|_| worktree.to_path_buf());
    let seed = format!("{}\u{0}{}", abs.display(), entry_name);
    let h = fnv1a_64(seed.as_bytes());
    49152 + (h % 16384) as u16
}

impl FromLua for Overrider {
    fn from_lua(value: Value, _lua: &Lua) -> LuaResult<Self> {
        let table = expect_table(value, "Overrider")?;
        let kind: String = table.get("__kind")?;
        match kind.as_str() {
            "port" => Ok(Overrider::Port),
            other => Err(mlua::Error::FromLuaConversionError {
                from: "table",
                to: "Overrider".to_string(),
                message: Some(format!("unknown overrider kind `{other}`")),
            }),
        }
    }
}
```

- [ ] **Step 4: Run the overrider tests to verify they pass**

Run: `cargo test --lib overrider`
Expected: PASS (8 tests). The rest of the crate will not compile yet — that is fixed in Step 5.

- [ ] **Step 5: Update the callers in `src/config/mod.rs`**

In `apply_cfg`, thread the entry name and dir through:
```rust
pub fn apply_cfg(cfg: &Cfg, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for (name, entry) in cfg {
        entry
            .overrider
            .apply(dir, name, &entry.locate_all(dir)?)?;
    }
    Ok(())
}
```

In `diff_cfg`, compute the real value instead of `ir_label()`:
```rust
pub fn diff_cfg(cfg: &Cfg, dir: &Path) -> Result<Vec<DiffEntry>, Box<dyn std::error::Error>> {
    let mut out = vec![];
    for (name, entry) in cfg {
        let applicators = entry.locate_all(dir)?;
        let new_value = entry.overrider.value(dir, name)?;
        for a in &applicators {
            let new_line = a.old_line.replacen(&a.old_value, &new_value, 1);
            out.push(DiffEntry {
                entry_name: name.clone(),
                path: a.path.clone(),
                line_number: a.line_number,
                old_value: a.old_value.clone(),
                new_value: new_value.clone(),
                old_line: a.old_line.clone(),
                new_line,
            });
        }
    }
    Ok(out)
}
```

In the `mod.rs` tests, update the two `override = { __kind = "random_port" }` strings to `override = { __kind = "port" }`, and update the `matches!(entry.overrider, Overrider::RandomPort)` assertion in `config_entry_deserializes` to `Overrider::Port`.

- [ ] **Step 6: Rename in the Fennel library and its test**

In `src/emux.fnl`, change the `:o` table to:
```fennel
   :o {:port {:__kind :port}}})
```

In `src/lua_api.rs`, rename the test `emux_o_rand_port_is_random_port_table` to `emux_o_port_is_port_table` and update its body:
```rust
    #[test]
    fn emux_o_port_is_port_table() {
        let lua = loaded_lua();
        let kind: String = lua.load(r#"emux.o.port.__kind"#).eval().unwrap();
        assert_eq!(kind, "port");
    }
```

- [ ] **Step 7: Add the idempotent-run integration test in `src/config/mod.rs`**

Add to the `tests` module in `src/config/mod.rs`:
```rust
    #[test]
    fn run_is_idempotent() {
        let dir = std::env::temp_dir().join("emux_test_idempotent");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".env"), "PORT=4327\n").unwrap();
        let path = dir.join("emux.lua");
        std::fs::write(
            &path,
            r#"
            return { ["api-port"] = {
                locate = { emux.l.envFile(".env", "PORT") },
                base = "4327",
                override = emux.o.port,
            } }
            "#,
        )
        .unwrap();
        let cfg = load_config_file(&path).unwrap();
        apply_cfg(&cfg, &dir).unwrap();
        let first = std::fs::read_to_string(dir.join(".env")).unwrap();
        apply_cfg(&cfg, &dir).unwrap();
        let second = std::fs::read_to_string(dir.join(".env")).unwrap();
        assert_eq!(first, second, "second run must not change the file");
        assert!(first.starts_with("PORT="));
        assert_ne!(first.trim(), "PORT=4327", "port should have been overridden");
    }
```
Note: this test references `base = "4327"`, which `load_config_file` must accept. `ConfigEntry::from_lua` reads `base` in Task 3; until then an unknown key is simply ignored by `table.get`, so this test passes now and stays valid.

- [ ] **Step 8: Run the full test suite**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src/config/overrider.rs src/config/mod.rs src/emux.fnl src/lua_api.rs
git commit -m "feat: deterministic per-worktree port override

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Add `base` and the `restore` command

**Files:**
- Modify: `src/config/mod.rs` (`ConfigEntry`, `FromLua`, add `restore_cfg`, tests)
- Create: `src/commands/restore.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `Cfg`, `ConfigEntry`, `Overrider::value`, `Applicator::apply`.
- Produces:
  - `ConfigEntry.base: Option<String>`
  - `pub fn restore_cfg(cfg: &Cfg, dir: &Path) -> Result<(), Box<dyn std::error::Error>>`
  - `pub fn commands::restore::run(file: PathBuf)`
  - `Commands::Restore { file: PathBuf }`

- [ ] **Step 1: Write the failing `base`/`restore` tests in `src/config/mod.rs`**

Add to the `tests` module in `src/config/mod.rs`:
```rust
    #[test]
    fn base_accepts_number() {
        let lua = Lua::new();
        let v = eval(
            &lua,
            r#"{ locate = {}, base = 4327, override = { __kind = "port" } }"#,
        );
        let entry = ConfigEntry::from_lua(v, &lua).unwrap();
        assert_eq!(entry.base.as_deref(), Some("4327"));
    }

    #[test]
    fn base_absent_is_none() {
        let lua = Lua::new();
        let v = eval(&lua, r#"{ locate = {}, override = { __kind = "port" } }"#);
        let entry = ConfigEntry::from_lua(v, &lua).unwrap();
        assert_eq!(entry.base, None);
    }

    #[test]
    fn restore_returns_to_base() {
        let dir = std::env::temp_dir().join("emux_test_restore_env");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".env"), "PORT=4327\n").unwrap();
        let path = dir.join("emux.lua");
        std::fs::write(
            &path,
            r#"
            return { ["api-port"] = {
                locate = { emux.l.envFile(".env", "PORT") },
                base = "4327",
                override = emux.o.port,
            } }
            "#,
        )
        .unwrap();
        let cfg = load_config_file(&path).unwrap();
        apply_cfg(&cfg, &dir).unwrap();
        assert_ne!(
            std::fs::read_to_string(dir.join(".env")).unwrap().trim(),
            "PORT=4327"
        );
        restore_cfg(&cfg, &dir).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join(".env")).unwrap(),
            "PORT=4327\n"
        );
    }

    #[test]
    fn restore_json_returns_to_base() {
        let dir = std::env::temp_dir().join("emux_test_restore_json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("env.json"), "{\n  \"port\": 4327\n}\n").unwrap();
        let path = dir.join("emux.lua");
        std::fs::write(
            &path,
            r#"
            return { ["api-port"] = {
                locate = { emux.l.jsonFile("env.json", ".port") },
                base = "4327",
                override = emux.o.port,
            } }
            "#,
        )
        .unwrap();
        let cfg = load_config_file(&path).unwrap();
        apply_cfg(&cfg, &dir).unwrap();
        restore_cfg(&cfg, &dir).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("env.json")).unwrap()).unwrap();
        assert_eq!(json["port"].as_u64().unwrap(), 4327);
    }

    #[test]
    fn restore_requires_base() {
        let dir = std::env::temp_dir().join("emux_test_restore_nobase");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".env"), "PORT=4327\n").unwrap();
        let path = dir.join("emux.lua");
        std::fs::write(
            &path,
            r#"
            return { ["api-port"] = {
                locate = { emux.l.envFile(".env", "PORT") },
                override = emux.o.port,
            } }
            "#,
        )
        .unwrap();
        let cfg = load_config_file(&path).unwrap();
        assert!(restore_cfg(&cfg, &dir).is_err());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib config`
Expected: FAIL to compile (`base` field and `restore_cfg` do not exist).

- [ ] **Step 3: Add the `base` field, parse it, and add `restore_cfg`**

In `src/config/mod.rs`, add `base` to the struct:
```rust
#[derive(Debug)]
pub struct ConfigEntry {
    pub locate: Vec<Locator>,
    pub base: Option<String>,
    pub overrider: Overrider, // `override` is a reserved keyword
}
```

In `impl FromLua for ConfigEntry`, read `base` (Lua coerces a number to its string form via `Option<String>`):
```rust
        let base: Option<String> = table.get("base")?;
        let overrider = Overrider::from_lua(table.get("override")?, lua)?;
        Ok(ConfigEntry { locate, base, overrider })
```

Add the `restore_cfg` function next to `apply_cfg`:
```rust
pub fn restore_cfg(cfg: &Cfg, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for (name, entry) in cfg {
        let base = entry
            .base
            .as_ref()
            .ok_or_else(|| format!("entry \"{name}\": restore requires a base value"))?;
        for a in &entry.locate_all(dir)? {
            a.apply(base)?;
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run the config tests to verify they pass**

Run: `cargo test --lib config`
Expected: PASS.

- [ ] **Step 5: Add the `restore` command**

Create `src/commands/restore.rs`:
```rust
use std::{path::PathBuf, process};

pub fn run(file: PathBuf) {
    let dir = super::parent_dir(&file);
    crate::config::load_config_file(&file)
        .and_then(|cfg| crate::config::restore_cfg(&cfg, &dir))
        .unwrap_or_else(|err| {
            eprintln!("error: {err}");
            process::exit(1);
        });
}
```

In `src/commands/mod.rs`, add the module declaration alongside the others:
```rust
pub mod diff;
pub mod restore;
pub mod run;
pub mod verify;
```

- [ ] **Step 6: Register the subcommand in `src/main.rs`**

Add a variant to the `Commands` enum after `Diff`:
```rust
    /// Restore files to their declared `base` values.
    Restore {
        /// Path to the Lua (.lua) or Fennel (.fnl) config file.
        file: PathBuf,
    },
```

Add the match arm in `main`:
```rust
        Commands::Restore { file } => commands::restore::run(file),
```

- [ ] **Step 7: Build and test**

Run: `cargo test && cargo run -- --help`
Expected: tests PASS; `--help` lists `verify`, `run`, `diff`, `restore`.

- [ ] **Step 8: Commit**

```bash
git add src/config/mod.rs src/commands/restore.rs src/commands/mod.rs src/main.rs
git commit -m "feat: add base field and stateless restore command

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Update examples, docs, and README

**Files:**
- Modify: `examples/emux.fnl`, `examples/emux.lua`, `examples/environment.local.json`
- Modify: `docs/emux.fnl`, `docs/emux.lua`
- Modify: `README.md`

**Interfaces:** none (no compiled code). Goal: every example runs against the new API and metadata matches.

- [ ] **Step 1: Update the example configs**

Replace `examples/emux.fnl` with:
```fennel
(local cfg
  {:api-port
   {:locate [(emux.l.envFile ".env" "PORT")
             (emux.l.jsonFile "environment.local.json" ".apiPort")]
    :base "4327"
    :override emux.o.port}
   :db-port
   {:locate [(emux.l.jsonFile "environment.local.json" ".dbPort")]
    :base "5432"
    :override emux.o.port}})

cfg
```

Replace `examples/emux.lua` with:
```lua
local cfg = {
  ["api-port"] = {
    locate = {
      emux.l.envFile(".env", "PORT"),
      emux.l.jsonFile("environment.local.json", ".apiPort"),
    },
    base = "4327",
    override = emux.o.port,
  },
  ["db-port"] = {
    locate = {
      emux.l.jsonFile("environment.local.json", ".dbPort"),
    },
    base = "5432",
    override = emux.o.port,
  },
}

return cfg
```

Replace `examples/environment.local.json` with a file that uses discrete port keys (the old `apiUrl` embedded the port in a string, which the key-anchored locators can no longer target):
```json
{
  "apiPort": 4327,
  "dbPort": 5432
}
```

- [ ] **Step 2: Verify the examples load and run**

Run:
```bash
cargo run -- verify examples/emux.fnl
cargo run -- verify examples/emux.lua
cargo run -- diff examples/emux.fnl
```
Expected: `verify` prints `ok` for both; `diff` shows `api-port` and `db-port` lines with computed ports (no `<random_port>` placeholder).

- [ ] **Step 3: Update the editor metadata docs**

Replace `docs/emux.lua` with:
```lua
---@meta

---@class Locator A pipeline of file filters.
---@field filters table[]

---@class Overrider A value-replacement strategy.

---@class EmuxLocators
---@field envFile fun(path: string, variable: string): Locator Target a variable in a dotenv-style file.
---@field jsonFile fun(path: string, selector: string): Locator Target a value at a dotted selector in a JSON file.

---@class EmuxOverriders
---@field port Overrider Replace located values with a deterministic per-worktree free-range port.

---@class EmuxLib The emux API available in all config files.
---@field l EmuxLocators Locator functions.
---@field o EmuxOverriders Overrider values.

---@class _G
---@field emux EmuxLib
```

In `docs/emux.fnl`, remove the `files` and `regex` field entries under `:l`, add a `jsonFile` entry, and rename the `:o` `randPort` field to `port`. The `:l` `:fields` table becomes:
```fennel
       {:envFile {:binding "emux.l.envFile"
                  :metadata {:fls/itemKind "Function"
                             :fnl/arglist ["path" "variable"]
                             :fnl/docstring "Return a locator that targets `variable` in a dotenv-style file at `path`.

Example:
  (emux.l.envFile \".env\" \"PORT\")"}}
        :jsonFile {:binding "emux.l.jsonFile"
                   :metadata {:fls/itemKind "Function"
                              :fnl/arglist ["path" "selector"]
                              :fnl/docstring "Return a locator that targets the value at a dotted selector in a JSON file.

Example:
  (emux.l.jsonFile \"env.json\" \".server.port\")"}}}}
```
And the `:o` `:fields` table becomes:
```fennel
       {:port {:binding "emux.o.port"
               :metadata {:fls/itemKind "Constant"
                          :fnl/docstring "Replace all located values with a deterministic per-worktree free-range port."}}}}}}}
```

- [ ] **Step 4: Update the README**

In `README.md`, replace the fenced Fennel example (the `(cfg :api-port ...)` block) and its surrounding explanation with the current API and a note about `restore`:
````markdown
For example, to override the port the API server runs on:

```fennel
(local cfg
  {:api-port
   {:locate [(emux.l.envFile ".env" "PORT")
             (emux.l.jsonFile "environment.local.json" ".apiPort")]
    :base "4327"
    :override emux.o.port}})

cfg
```

Each entry declares where the value lives (`locate`), its original value
(`base`), and how to replace it (`override`). `emux run config.fnl` writes a
deterministic per-worktree port to every location; re-running is a no-op.
`emux restore config.fnl` writes each `base` value back. Because the override
is a pure function of the worktree path and the values come from the config,
emux needs no state file.
````

- [ ] **Step 5: Final full check**

Run: `cargo test && cargo run -- diff examples/emux.fnl && cargo run -- restore examples/emux.fnl`
Expected: tests PASS; `diff` shows computed ports; `restore` exits 0 and leaves `examples/.env` / `environment.local.json` at their base values (`PORT=4327`, `apiPort: 4327`, `dbPort: 5432`).

- [ ] **Step 6: Commit**

```bash
git add examples docs README.md
git commit -m "docs: update examples, metadata, and README for port/base/restore

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:**
- Deterministic override + rename → Task 2. ✔
- Per-entry `base`, required by restore, not read by run → Task 3 (`restore_cfg` reads base; `apply_cfg` does not). ✔
- `restore` command → Task 3. ✔
- `diff` shows real value → Task 2 Step 5. ✔
- Delete `regex`/`files` + drop `glob`/`grep` → Task 1. ✔
- Update examples/docs/README → Task 4. ✔
- Out of scope (collision avoidance, embedded-substring, new overriders) → not implemented; example `apiUrl` replaced with discrete keys, noted in Task 4 Step 1. ✔
- Testing: determinism, range, idempotent run, restore round-trip (env + json), missing base, base coercion → Tasks 2–3. ✔

**Placeholder scan:** none — every code and test step contains complete code.

**Type consistency:** `Overrider::Port`, `value(worktree, entry_name)`, `apply(worktree, entry_name, applicators)`, `restore_cfg(cfg, dir)`, `ConfigEntry.base: Option<String>`, `Commands::Restore { file }` are used consistently across Tasks 2–3 and the call sites in `mod.rs`/`main.rs`.
