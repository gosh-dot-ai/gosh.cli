// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::io::BufRead;
use std::io::BufReader;
use std::io::Seek;
use std::io::SeekFrom;

use clap::Args;
use colored::Colorize;

use crate::context::AppContext;

#[derive(Args)]
pub struct LogsArgs {
    /// Service name (all if omitted)
    pub service: Option<String>,
    /// Follow log output
    #[arg(short, long)]
    pub follow: bool,
    /// Number of lines to show
    #[arg(short = 'n', long, default_value = "50")]
    pub lines: usize,
}

pub fn run(ctx: &AppContext, args: &LogsArgs) -> anyhow::Result<()> {
    let cfg = &ctx.services;
    let services: Vec<String> = if let Some(name) = &args.service {
        if !cfg.services.contains_key(name.as_str()) {
            anyhow::bail!("unknown service: {name}");
        }
        vec![name.to_string()]
    } else {
        cfg.start_order()
    };

    if args.follow {
        tail_follow(ctx, &services)?;
    } else {
        tail_last(ctx, &services, args.lines)?;
    }

    Ok(())
}

fn tail_last(ctx: &AppContext, services: &[String], n: usize) -> anyhow::Result<()> {
    let multi = services.len() > 1;
    for name in services {
        let path = ctx.log_file(name);
        if !path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&path)?;
        let all_lines: Vec<&str> = content.lines().collect();
        let start = all_lines.len().saturating_sub(n);
        for line in &all_lines[start..] {
            if multi {
                println!("{} {}", format!("[{name}]").cyan(), line);
            } else {
                println!("{line}");
            }
        }
    }
    Ok(())
}

fn tail_follow(ctx: &AppContext, services: &[String]) -> anyhow::Result<()> {
    use std::time::Duration;

    let multi = services.len() > 1;
    let mut readers: Vec<(String, BufReader<std::fs::File>)> = Vec::new();

    for name in services {
        let path = ctx.log_file(name);
        if !path.exists() {
            continue;
        }
        let mut file = std::fs::File::open(&path)?;
        file.seek(SeekFrom::End(0))?;
        readers.push((name.clone(), BufReader::new(file)));
    }

    if readers.is_empty() {
        println!("No log files found.");
        return Ok(());
    }

    println!("Following logs... (Ctrl+C to stop)");

    loop {
        let mut any = false;
        for (name, reader) in &mut readers {
            let mut line = String::new();
            while reader.read_line(&mut line)? > 0 {
                let trimmed = line.trim_end();
                if multi {
                    println!("{} {}", format!("[{name}]").cyan(), trimmed);
                } else {
                    println!("{trimmed}");
                }
                line.clear();
                any = true;
            }
        }
        if !any {
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}
