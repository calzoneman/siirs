use std::env;

use anyhow::{Result, bail};
use siirs::achievements::{self, AchievementStatus, RequirementStatus};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        bail!("Usage: {} <path to game.sii> <path to game folder>", args[0]);
    }

    let results = achievements::get_achievement_status(&args[1], &args[2])?;
    for status in results {
        print_results(&status)
    }

    Ok(())
}

fn print_results(status: &AchievementStatus) {
    print_boxed(&status.name);
    for req in &status.requirements {
        let prefix = if req.status == RequirementStatus::Completed {
            "\x1b[1;32m✓\x1b[1;30m "
        } else {
            "\x1b[0m  "
        };

        println!("{} {}: {}", prefix, req.progress_description, req.name);
    }

    println!("\x1b[0m")
}

fn print_boxed(s: &str) {
    println!("╭─{}─╮", "─".repeat(s.len()));
    println!("│ {} │", s);
    println!("╰─{}─╯", "─".repeat(s.len()));
}