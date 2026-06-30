pub mod diff;
pub mod restore;
pub mod run;
pub mod verify;

use std::path::{Path, PathBuf};

fn parent_dir(file: &Path) -> PathBuf {
    file.parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}
