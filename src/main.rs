use std::collections::HashMap;
use std::env;
use std::os::unix::process::CommandExt;
use std::process::{self, Command};

use ratatui::style::Color;

mod config;
mod tui;

use config::load_config;

pub(crate) const ACCENT: Color = Color::Cyan;
pub(crate) const MUTED: Color = Color::DarkGray;

pub(crate) fn fatal(msg: &str) -> ! {
    eprintln!("\x1b[31merror\x1b[0m: {msg}");
    process::exit(1);
}

fn launch(profile: &config::Profile, profile_name: &str) -> ! {
    let mut env_map: HashMap<String, String> = env::vars()
        .filter(|(k, _)| !k.starts_with("ANTHROPIC_"))
        .collect();

    // Built last over the (ANTHROPIC_-filtered) shell env above, so profile
    // values — including the `env` table — win over exported variables.
    env_map.extend(config::build_env(profile, true));

    let user_args: Vec<String> = env::args().skip(1).filter(|a| a != profile_name).collect();

    let err = Command::new("claude")
        .args(&user_args)
        .stdin(process::Stdio::inherit())
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .env_clear()
        .envs(&env_map)
        .exec();

    fatal(&format!("failed to launch claude: {err}"));
}

fn main() {
    let config = load_config();
    let args: Vec<String> = env::args().collect();

    // If a profile name is given, launch it directly — skip the TUI.
    if args.len() > 1 && !args[1].starts_with('-') {
        let name = &args[1];
        match config.profiles.get(name) {
            Some(profile) => launch(profile, name),
            None => fatal(&format!("unknown profile \"{name}\"")),
        }
    }

    let app = tui::App::new(config);
    let (app, selected) = tui::run(app);
    match selected {
        Some(name) => {
            let config = app.into_config();
            match config.profiles.get(&name) {
                Some(profile) => launch(profile, &name),
                None => fatal(&format!("unknown profile \"{name}\"")),
            }
        }
        None => {
            println!("no profile selected, exiting");
        }
    }
}
