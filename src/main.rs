use clap::Parser;
use futures::stream::{self, StreamExt};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::process::{Command, exit};

#[derive(Parser)]
#[command(name = "envgg")]
#[command(about = "Run commands with environment variables from .env, .env.development, .env.staging, or .env.production", long_about = None)]
struct Cli {
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        required = true,
        help = "Arguments: [env] command...\n\nWhere env is optional and can be: d, development, s, staging, p, production\n\nExamples:\nenvgg npm start          # .env\nenvgg d npm start        # .env.development\nenvgg p tsx src/index.ts # .env.production"
    )]
    args: Vec<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

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
        eprintln!("Error: No command specified");
        exit(1);
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
    let env_vars = match read_env_file(&env_path).await {
        Ok(vars) => vars,
        Err(e) => {
            eprintln!("Error reading {}: {}", env_path.to_string_lossy(), e);
            exit(1);
        }
    };

    // Execute the command with environment variables
    let status = Command::new(&command[0])
        .args(&command[1..])
        .envs(env_vars)
        .status();

    match status {
        Ok(exit_status) => {
            exit(exit_status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("Error executing command: {}", e);
            exit(1);
        }
    }
}

async fn read_env_file(path: &PathBuf) -> anyhow::Result<Vec<(String, String)>> {
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

    let env_vars = stream::iter(lines)
        .filter_map(|line| async move {
            let trimmed = line.trim();

            // Skip empty lines and comments (lines starting with #)
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }

            // Case 1: KEY=VALUE format
            if let Some(pos) = trimmed.find('=') {
                let key = trimmed[..pos].trim().to_string();
                let value = trimmed[pos + 1..].trim().to_string();

                // Remove quotes if present
                let value = if (value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\''))
                {
                    value[1..value.len() - 1].to_string()
                } else {
                    value
                };

                Some((key, value))
            } else {
                // Case 2: KEY only (no =) - fetch from keyring
                let key = trimmed.to_string();
                match get_secret_from_keyring(&key).await {
                    Ok(value) => Some((key, value)),
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to get secret for '{}' from keyring: {}",
                            key, e
                        );
                        eprintln!("Skipping this environment variable.");
                        None
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .await;

    Ok(env_vars)
}

async fn get_secret_from_keyring(target: &str) -> anyhow::Result<String> {
    let ss = secret_service::SecretService::connect(secret_service::EncryptionType::Dh).await?;
    let collection = ss.get_default_collection().await?;
    let notes = collection
        .search_items(HashMap::from([
            ("account", target),
            ("service", "Env"),
            ("xdg:schema", "org.freedesktop.Secret.Generic"),
        ]))
        .await?;

    match notes.len() {
        0 => Err(anyhow::anyhow!("Secret not found")),
        1 => {
            let secret = notes.first().expect("???").get_secret().await?;
            Ok(String::from_utf8(secret)?)
        }
        n => Err(anyhow::anyhow!("Multiple matches: {}", n)),
    }
}
