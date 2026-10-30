use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::process::{self, Command};

use clap::{error::ErrorKind, Command as CliCommand};

mod config;

pub(crate) fn fatal(msg: &str) -> ! {
    eprintln!("error: {msg}");
    process::exit(1);
}

fn cli() -> CliCommand {
    CliCommand::new("ccs")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Launch Claude Code with a named profile")
        .allow_external_subcommands(true)
        .disable_help_subcommand(true)
        .override_usage("ccs [OPTIONS] [PROFILE] [CLAUDE_ARGS]...")
        .after_help(
            "Run without arguments to list available profiles.\n\
             Arguments: profile args first, then command-line args unchanged.\n\n\
             Config: ~/.config/ccs/config.toml\n\n\
             Examples:\n  ccs\n  ccs my-profile\n  ccs my-profile -p \"hello\"",
        )
}

fn list_profiles(config: &config::Config) {
    let mut profiles: Vec<_> = config.profiles.iter().collect();
    profiles.sort_by_key(|(name, _)| *name);

    println!("\nProfiles:");
    if profiles.is_empty() {
        println!("  No profiles configured. Add one to ~/.config/ccs/config.toml.");
        return;
    }
    for (name, profile) in profiles {
        match profile.description.as_deref().filter(|s| !s.is_empty()) {
            Some(description) => println!("  {name}  {description}"),
            None => println!("  {name}"),
        }
    }
}

fn launch(
    settings: &config::Settings,
    profile_name: &str,
    user_args: impl Iterator<Item = OsString>,
) -> ! {
    let mut env_map: HashMap<String, String> = env::vars()
        .filter(|(k, _)| !k.starts_with("ANTHROPIC_"))
        .collect();

    // Resolve references from the original environment before applying profile values.
    env_map.extend(
        settings
            .build_env(profile_name)
            .unwrap_or_else(|e| fatal(&e)),
    );

    let err = Command::new("claude")
        .args(settings.build_args())
        .args(user_args)
        .env_clear()
        .envs(&env_map)
        .exec();

    fatal(&format!("failed to launch claude: {err}"));
}

fn main() {
    let mut command = cli();
    let matches = command.get_matches_mut();
    let config = config::load_config();
    let Some((name, args)) = matches.subcommand() else {
        command
            .print_help()
            .unwrap_or_else(|e| fatal(&e.to_string()));
        println!();
        list_profiles(&config);
        return;
    };

    let profile = config.profiles.get(name).unwrap_or_else(|| {
        command
            .error(
                ErrorKind::InvalidValue,
                format!("unknown profile \"{name}\"; run ccs to list available profiles"),
            )
            .exit()
    });
    let settings = config.resolve(profile, name).unwrap_or_else(|e| fatal(&e));
    let args = args.get_many::<OsString>("").into_iter().flatten().cloned();
    launch(&settings, name, args);
}
