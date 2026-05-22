use std::{path::PathBuf, process};

pub fn run(file: PathBuf) {
    let dir = file
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    crate::config::load_config_file(&file)
        .and_then(|cfg| crate::config::apply_cfg(&cfg, &dir))
        .unwrap_or_else(|err| {
            eprintln!("error: {err}");
            process::exit(1);
        });
}
