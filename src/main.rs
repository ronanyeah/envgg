use clap::Parser;
use envgg::{
    EnvLine, get_env_var_names_from_file, get_secret_from_keyring, list_secret_labels,
    read_env_file, ui,
};
use futures::stream::{self, StreamExt};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "envgg")]
#[command(about = "Run commands with environment variables from .env, .env.development, .env.staging, or .env.production", long_about = None)]
struct Cli {
    #[arg(
        short = 'l',
        long = "list",
        help = "List all secrets stored in the `envgg` namespace in system keyring"
    )]
    list: bool,

    #[arg(short = 'o', long = "open", help = "Open the GUI manager")]
    open: bool,

    #[arg(
        short = 'c',
        long = "current",
        help = "Print available environment variable names from suppported .env files in current folder"
    )]
    current: bool,

    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        required = false,
        help = "Arguments: [env] command...

Where env is optional and can be: [d, development, s, staging, p, production]

Examples:
envgg npm start             # .env
envgg development npm start # .env.development
envgg d npm start           # .env.development
envgg p tsx src/index.ts    # .env.production"
    )]
    args: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    keyring_core::set_default_store(dbus_secret_service_keyring_store::Store::new()?);

    #[cfg(target_os = "macos")]
    keyring_core::set_default_store(apple_native_keyring_store::keychain::Store::new()?);

    #[cfg(target_os = "windows")]
    keyring_core::set_default_store(windows_native_keyring_store::keychain::Store::new()?);

    let cli = Cli::parse();

    // Handle list flag
    if cli.list {
        match list_secret_labels() {
            Ok(secrets) => {
                for label in secrets {
                    println!("{}", label);
                }
                return Ok(());
            }
            Err(e) => {
                anyhow::bail!("Error listing secrets: {}", e);
            }
        }
    }

    // Handle open flag
    if cli.open {
        ui::open_secrets_viewer().await;
        return Ok(());
    }

    // Handle current flag
    if cli.current {
        let mut env_files = vec![
            PathBuf::from(".env"),
            PathBuf::from(".env.development"),
            PathBuf::from(".env.staging"),
            PathBuf::from(".env.production"),
        ];

        env_files.retain(|f| f.exists());

        if env_files.is_empty() {
            println!("No .env files found in current directory");
        } else {
            println!("{} .env file(s) found", env_files.len());
            for path in env_files {
                let Some(name) = path.file_name().and_then(|f| f.to_str()) else {
                    continue;
                };
                if path.exists() {
                    match get_env_var_names_from_file(&path) {
                        Ok(var_names) => {
                            if var_names.is_empty() {
                                println!("\n{}: No variables", name);
                            } else {
                                println!("\n{}:", name);
                                for var_name in var_names {
                                    println!("{}", var_name);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error reading {}: {}", name, e);
                        }
                    }
                }
            }
        };

        return Ok(());
    }

    // Check if first argument is an environment specifier
    let valid_envs = ["d", "development", "s", "staging", "p", "production"];
    let (env, command) = if !cli.args.is_empty() && valid_envs.contains(&cli.args[0].as_str()) {
        // First arg is an environment
        (Some(cli.args[0].clone()), &cli.args[1..])
    } else {
        // No environment specified, all args are the command
        (None, &cli.args[..])
    };

    if command.is_empty() {
        anyhow::bail!("Error: No command specified");
    }

    // Construct the env file path based on whether an environment was specified
    let env_path = match env {
        None => {
            // No environment specified, use .env
            PathBuf::from(".env")
        }
        Some(env) => {
            // Normalize short form to long form
            let env_name = match env.as_str() {
                "d" => "development",
                "s" => "staging",
                "p" => "production",
                _ => &env,
            };
            // Use .env.{environment}
            PathBuf::from(format!(".env.{}", env_name))
        }
    };

    // Read and parse the env file
    let env_vars = process_env_file(&env_path).await?;

    // Execute the command with environment variables
    Command::new(&command[0])
        .args(&command[1..])
        .envs(env_vars)
        .status()?;

    Ok(())
}

// If duplicate labels exist, the last entry will take precedence
async fn process_env_file(path: &PathBuf) -> anyhow::Result<Vec<(String, String)>> {
    let lines = read_env_file(path)?;

    let env_map = stream::iter(lines)
        .filter_map(|line| async move {
            match line {
                EnvLine::Comment => None,
                EnvLine::Direct { key, value } => Some((key, value)),
                EnvLine::Alias { key, keyring_key } => {
                    match get_secret_from_keyring(&keyring_key) {
                        Ok(secret_value) => Some((key, secret_value)),
                        Err(e) => {
                            eprintln!(
                                "Warning: Failed to get secret for '{}' from keyring: {}",
                                keyring_key, e
                            );
                            eprintln!("Skipping environment variable '{}'.", key);
                            None
                        }
                    }
                }
                EnvLine::Lookup { key } => match get_secret_from_keyring(&key) {
                    Ok(value) => Some((key, value)),
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to get secret for '{}' from keyring: {}",
                            key, e
                        );
                        eprintln!("Skipping this environment variable.");
                        None
                    }
                },
            }
        })
        .collect::<HashMap<_, _>>()
        .await;

    Ok(env_map.into_iter().collect())
}
