use std::{path::PathBuf, process};

pub fn run(file: PathBuf) {
    let dir = super::parent_dir(&file);
    crate::config::load_config_file(&file)
        .and_then(|cfg| crate::config::apply_cfg(&cfg, &dir))
        .unwrap_or_else(|err| {
            eprintln!("error: {err}");
            process::exit(1);
        });
}
