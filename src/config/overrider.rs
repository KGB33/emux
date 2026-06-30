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
