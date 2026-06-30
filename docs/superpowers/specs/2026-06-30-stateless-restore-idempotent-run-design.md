# Stateless Restore + Idempotent Run

**Date:** 2026-06-30
**Status:** Approved (design)

## Problem

emux applies environment overrides to files in a worktree, but two gaps undermine
its purpose:

1. **No restore/undo.** `run` overwrites files irreversibly. There is no record of
   the original values and no way to revert.
2. **Not idempotent.** Overrides are generated randomly (`randPort` binds to port 0).
   Re-running produces *different* values, and value-anchored locators can no longer
   find the original to replace. Running emux twice on a worktree is undefined.

## Constraints

- **No sidecar state file.** Everything emux needs to apply *or* undo must be
  computable from the config file plus the worktree itself. No `.emux/state.json`,
  no lockfile.

## Core idea

Two changes make both `run` and `restore` pure functions of `config + worktree`,
with nothing persisted:

1. **Deterministic overrides.** The override value is a stable function of the
   worktree identity and the entry name, so re-running recomputes the *same* value.
   `Dv = override(worktree_path, entry_name)`.
2. **Config-declared base.** Each entry declares its original (`base`) value. That is
   the value `restore` writes back. The config becomes the complete two-way source of
   truth.

Because the remaining locators are **key-anchored** (they find their position by
dotenv key or JSON selector, not by current value), locating works identically
whether the file currently holds the base value (fresh) or `Dv` (already run). This
eliminates the need to search for "the old value" at all.

## Scope

### In scope

- Make the port overrider deterministic; rename `randPort` -> `port`.
- Add a per-entry `base` field used by `restore`.
- Add the `restore` command.
- Update `diff` to show the real computed value instead of the `<random_port>`
  placeholder.
- **Delete** the `regex` and `files` locators (`Filter::Regex`, `Filter::File`,
  `emux.l.regex`, `emux.l.files`, `search_regex`, and the glob-driven path in
  `Locator::locate`).
- Drop the now-unused `glob` and `grep` dependencies from `Cargo.toml`.
- Update `examples/` and `docs/` (`emux.fnl`, `emux.lua`, fennel-ls/lua-ls metadata)
  to the new API.

### Out of scope (explicitly)

- **Cross-worktree uniqueness / collision avoidance.** A deterministic port mapped
  into the ephemeral range can, in principle, collide with another worktree or a
  running service. Guaranteeing uniqueness needs a shared registry — tracked
  separately as issue #4. This design accepts low-probability collisions.
- **Replacing a value embedded inside a larger string** (e.g. the port inside
  `"apiUrl": "http://localhost:4327"`). The deleted `regex`/`files` locators were the
  only way to do this. A dedicated locator may be added later; not now.
- New overriders beyond `port`.

## Detailed design

### Locators (key-anchored only)

After this change there are exactly two locator kinds, both of which resolve a
position independent of the current value:

- `envFile(path, variable)` — the value after `variable=` in a dotenv-style file.
- `jsonFile(path, selector)` — the value at a dotted selector in a JSON file.

`Filter::Regex` and `Filter::File` are removed. `Locator::locate` no longer has a
glob-collect step; each remaining filter is self-contained and resolves directly
against its own path.

### Config schema

`ConfigEntry` gains a `base` field:

```fennel
{:api-port
 {:locate [(emux.l.envFile ".env" "PORT")
           (emux.l.jsonFile "environment.local.json" ".apiPort")]
  :base "4327"
  :override emux.o.port}}
```

- `base` is read as a string. A Lua number (`4327`) is coerced to its string form
  (`"4327"`); a Lua string is taken verbatim. This mirrors how values appear in
  files and how the JSON writer already parses a value back (`"4327"` -> JSON number
  `4327`, non-numeric -> JSON string).
- `base` is **optional at parse time** but **required by `restore`**: if `restore`
  reaches an entry with no `base`, it errors naming that entry. `run` never reads
  `base`.

### Deterministic override

`Overrider::Port` (renamed from `RandomPort`) computes:

```
seed = abs_path(worktree_dir) + "\0" + entry_name
h    = fnv1a_64(seed)
port = 49152 + (h % 16384)      // ephemeral range 49152..=65535
Dv   = port.to_string()
```

- **FNV-1a (64-bit), implemented locally** — not `std::hash::DefaultHasher`. We need
  the value to be stable across processes and machines for a given path; the std
  hasher's keying is an implementation detail we should not rely on.
- `worktree_dir` is the config file's parent directory (`commands::parent_dir`),
  canonicalized to an absolute path. The entry name salts the hash so different
  entries in the same worktree get different ports.
- The `ir_label()` method and the `<random_port>` placeholder are removed.
- `Overrider::value(worktree_dir, entry_name) -> Result<String>` replaces the old
  `generate()`. `apply` is threaded the worktree dir and entry name.

### Commands

| Command | Behavior |
|---|---|
| `verify <file>` | Unchanged. |
| `run <file>` | For each entry: compute `Dv`; write `Dv` at every located position. Idempotent — a second run recomputes the same `Dv` and rewrites the same bytes. Does not read `base`. |
| `restore <file>` | For each entry: require `base`; write `base` at every located position. Idempotent. |
| `diff <file>` | For each entry: show each location's current value -> `Dv`, using the real computed value (no placeholder). |

`run`, `restore`, and `diff` share the same locate step; they differ only in the
value written/shown. Apply logic is factored so `run` and `restore` are one function
parameterized by "value to write" (`Dv` vs `base`).

### Error handling

- `restore` on an entry with no `base` -> error: `entry "<name>": restore requires a base value`.
- Zero located positions (e.g. key already absent, or already in the target state)
  -> no-op, **not** an error. Keeps `run` and `restore` idempotent.
- Existing IO / JSON / config-parse errors are unchanged.

## Testing

Unit + integration tests covering:

- **Determinism:** same `(path, entry)` -> same value across calls; different path or
  different entry -> different value. FNV-1a output is stable for a known input.
- **Range:** computed port is always within `49152..=65535`.
- **Idempotent run:** write a fixture, `run` twice, assert the file is byte-identical
  after the first and second run.
- **Restore round-trip:** `run` then `restore` returns each located value to `base`.
- **Run/restore/run:** stable and consistent across a full cycle.
- **Per locator:** `envFile` and `jsonFile` each covered for run and restore.
- **base coercion:** numeric `base` and string `base` both work, including JSON
  numeric vs string write-back.
- **Missing base:** `restore` errors; `run` succeeds.

Existing regex/files/glob tests are deleted along with the code they cover.

## Migration / cleanup

- `Cargo.toml`: remove `glob` and `grep`.
- `examples/emux.fnl`, `examples/emux.lua`: drop the regex locator; add `:base`;
  rename `randPort` -> `port`. Target `.env` `PORT` and a JSON selector directly.
- `docs/emux.fnl` (fennel-ls metadata) and `docs/emux.lua` (lua-ls `---@meta`):
  remove `files`/`regex`, rename `randPort` -> `port`, document `base`.
- `README.md`: the conceptual example uses `regex`; update it to the new API and
  mention `restore`. (Light touch — full README overhaul is separate.)
- `src/main.rs`: register the `restore` subcommand.

## Open questions

None. All design decisions are resolved.
