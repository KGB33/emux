use colored::Colorize;
use std::{path::PathBuf, process};

pub fn run(file: PathBuf) {
    let dir = super::parent_dir(&file);
    crate::config::load_config_file(&file)
        .and_then(|cfg| crate::config::diff_cfg(&cfg, &dir))
        .map(|entries| {
            for e in &entries {
                println!(
                    "{}",
                    format!("[{}] {}:{}", e.entry_name, e.path.display(), e.line_number)
                        .bold()
                        .cyan()
                );
                println!("-  {}", color_span(&e.old_line, &e.old_value, |s| s.red()));
                println!(
                    "+  {}",
                    color_span(&e.new_line, &e.new_value, |s| s.green())
                );
            }
        })
        .unwrap_or_else(|err| {
            eprintln!("error: {err}");
            process::exit(1);
        });
}

fn color_span(line: &str, span: &str, colorize: impl Fn(&str) -> colored::ColoredString) -> String {
    match line.find(span) {
        Some(pos) => format!(
            "{}{}{}",
            &line[..pos],
            colorize(&line[pos..pos + span.len()]),
            &line[pos + span.len()..]
        ),
        None => line.to_owned(),
    }
}
