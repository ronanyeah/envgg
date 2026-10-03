use anyhow::Context;
use clap::{Args, Parser, Subcommand, ValueEnum};
use envgg::{
    EnvLine, add_secret_to_keyring, delete_secret_from_keyring, export_secrets,
    get_env_var_names_from_file, get_secret_from_keyring, is_valid_env_var_name,
    list_secret_labels, read_env_file, ui,
};
use futures::stream::{self, StreamExt};
use std::collections::HashMap;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

#[derive(Parser)]
#[command(
    version,
    about,
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true,
    arg_required_else_help = true,
    after_help = "Examples:
  envgg -- npm start                            # .env
  envgg development -- npm start                # .env.development
  envgg p -- tsx src/index.ts                   # .env.production
  envgg --env-file .my-unique-env -- npm start  # a specific env file
  envgg run p -- tsx src/index.ts               # same as without `run`

Exit codes when running a command:
  its own    the command ran
  125        envgg failed before running it
  126        the command was found but could not be run
  127        the command was not found"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,

    #[command(flatten)]
    run: RunArgs,
}

#[derive(Args)]
struct RunArgs {
    /// Environment to load from .env.<ENV>, also as d, s, p, t or l [default: .env]
    env: Option<Env>,

    /// Load this env file instead of .env or .env.<ENV>
    #[arg(long, value_name = "FILE", conflicts_with = "env")]
    env_file: Option<PathBuf>,

    /// Command and arguments to run (after `--`)
    #[arg(last = true, required = true)]
    cmd: Vec<String>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a command with the variables from a .env file (same as omitting `run`)
    Run(RunArgs),

    /// List the secrets stored in the `envgg` namespace of the system keyring
    Secrets,

    /// Add or update a secret (the value is prompted for, or read from stdin if piped)
    Set {
        /// Secret name, in UPPER_SNAKE_CASE
        name: String,
    },

    /// Delete a secret
    Delete {
        /// Name of the secret to delete
        name: String,

        /// Delete without asking for confirmation
        #[arg(short, long)]
        yes: bool,
    },

    /// Open the GUI manager
    Open,

    /// Print the variable names used by the .env files in the current folder
    Vars,

    /// Write all secrets as plaintext to a file
    Export {
        /// File to write
        #[arg(default_value = ".env.bak")]
        file: PathBuf,

        /// Overwrite the file if it already exists
        #[arg(short, long)]
        force: bool,
    },

    /// Print this CLI's help as Markdown, used to generate README.md
    #[command(hide = true)]
    MarkdownHelp,
}

#[derive(Clone, Copy, ValueEnum)]
enum Env {
    #[value(alias = "d")]
    Development,
    #[value(alias = "s")]
    Staging,
    #[value(alias = "p")]
    Production,
    #[value(alias = "t")]
    Test,
    #[value(alias = "l")]
    Local,
}

impl Env {
    fn file(self) -> PathBuf {
        PathBuf::from(match self {
            Env::Development => ".env.development",
            Env::Staging => ".env.staging",
            Env::Production => ".env.production",
            Env::Test => ".env.test",
            Env::Local => ".env.local",
        })
    }
}

/// The command itself couldn't be started
#[derive(Debug)]
struct SpawnError(io::Error);

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for SpawnError {}

// Same convention as env(1) and timeout(1)
fn exit_code(error: &anyhow::Error, runs_command: bool) -> u8 {
    match error.downcast_ref::<SpawnError>() {
        Some(SpawnError(e)) if e.kind() == io::ErrorKind::NotFound => 127,
        Some(_) => 126,
        None if runs_command => 125,
        None => 1,
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let Cli { command, run } = Cli::parse();
    let runs_command = matches!(command, None | Some(Cmd::Run(_)));

    match dispatch(command, run).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error:?}");
            ExitCode::from(exit_code(&error, runs_command))
        }
    }
}

async fn dispatch(command: Option<Cmd>, run_args: RunArgs) -> anyhow::Result<()> {
    // Doesn't need the keyring, so it also works where none is available
    if matches!(command, Some(Cmd::MarkdownHelp)) {
        let options = clap_markdown::MarkdownOptions::new()
            .title("envgg".to_string())
            .show_footer(false);
        print!("{}", clap_markdown::help_markdown_custom::<Cli>(&options));
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    keyring_core::set_default_store(dbus_secret_service_keyring_store::Store::new()?);

    #[cfg(target_os = "macos")]
    keyring_core::set_default_store(apple_native_keyring_store::keychain::Store::new()?);

    #[cfg(target_os = "windows")]
    keyring_core::set_default_store(windows_native_keyring_store::store::Store::new()?);

    match command {
        None => run(run_args).await,
        Some(Cmd::Run(args)) => run(args).await,
        Some(Cmd::Secrets) => {
            for label in list_secret_labels().context("failed to list secrets")? {
                println!("{label}");
            }
            Ok(())
        }
        Some(Cmd::Set { name }) => set_secret(&name),
        Some(Cmd::Delete { name, yes }) => delete_secret(&name, yes),
        Some(Cmd::Open) => {
            ui::open_secrets_viewer().await;
            Ok(())
        }
        Some(Cmd::Vars) => {
            print_vars();
            Ok(())
        }
        Some(Cmd::Export { file, force }) => {
            let count = export_secrets(&file, force)?;
            println!("Exported {count} secret(s) to {}", file.display());
            Ok(())
        }
        Some(Cmd::MarkdownHelp) => unreachable!("handled before the keyring is set up"),
    }
}

async fn run(RunArgs { env, env_file, cmd }: RunArgs) -> anyhow::Result<()> {
    let (program, args) = cmd
        .split_first()
        .context("no command specified, expected: envgg [ENV] -- <CMD>...")?;

    let env_path = match (env_file, env) {
        (Some(path), _) => path,
        (None, Some(env)) => env.file(),
        (None, None) => PathBuf::from(".env"),
    };
    anyhow::ensure!(
        env_path.exists(),
        "env file '{}' not found",
        env_path.display()
    );
    let env_vars = process_env_file(&env_path).await?;

    let mut command = Command::new(program);
    command.args(args).envs(env_vars);
    exec(command, program)
}

// Replaces this process, so signals and exit codes are the command's own
#[cfg(unix)]
fn exec(mut command: Command, program: &str) -> anyhow::Result<()> {
    use std::os::unix::process::CommandExt;
    // Only returns if the command couldn't be started
    Err(spawn_failure(program, command.exec()))
}

// No exec on this platform, so wait and pass the exit code through
#[cfg(not(unix))]
fn exec(mut command: Command, program: &str) -> anyhow::Result<()> {
    let status = command.status().map_err(|e| spawn_failure(program, e))?;
    std::process::exit(status.code().unwrap_or(1))
}

fn spawn_failure(program: &str, error: io::Error) -> anyhow::Error {
    anyhow::Error::new(SpawnError(error)).context(format!("failed to run '{program}'"))
}

// The value is never an argument, so it stays out of shell history and `ps`
fn set_secret(name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        is_valid_env_var_name(name),
        "'{name}' is not a valid name, use UPPER_SNAKE_CASE"
    );

    let value = if io::stdin().is_terminal() {
        rpassword::prompt_password(format!("Value for {name}: "))?
    } else {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        // Piping adds one trailing newline that isn't part of the value
        let value = input.strip_suffix('\n').unwrap_or(&input);
        value.strip_suffix('\r').unwrap_or(value).to_string()
    };
    anyhow::ensure!(!value.is_empty(), "empty value, nothing stored");

    add_secret_to_keyring(name, &value).with_context(|| format!("failed to store '{name}'"))?;
    println!("Stored '{name}'");
    Ok(())
}

fn delete_secret(name: &str, yes: bool) -> anyhow::Result<()> {
    if !yes {
        anyhow::ensure!(
            io::stdin().is_terminal(),
            "not a terminal, pass --yes to delete without confirmation"
        );
        eprint!("Delete '{name}'? [y/N] ");
        io::stderr().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            println!("Aborted");
            return Ok(());
        }
    }

    delete_secret_from_keyring(name).with_context(|| format!("failed to delete '{name}'"))?;
    println!("Deleted '{name}'");
    Ok(())
}

fn print_vars() {
    let env_files: Vec<PathBuf> = [".env"]
        .map(PathBuf::from)
        .into_iter()
        .chain(Env::value_variants().iter().map(|env| env.file()))
        .filter(|path| path.exists())
        .collect();

    if env_files.is_empty() {
        println!("No .env files found in current directory");
        return;
    }

    println!("{} .env file(s) found", env_files.len());
    for path in env_files {
        let Some(name) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        match get_env_var_names_from_file(&path) {
            Ok(var_names) if var_names.is_empty() => println!("\n{name}: No variables"),
            Ok(var_names) => {
                println!("\n{name}:");
                for var_name in var_names {
                    println!("{var_name}");
                }
            }
            Err(e) => eprintln!("Error reading {name}: {e}"),
        }
    }
}

// If duplicate labels exist, the last entry will take precedence
async fn process_env_file(path: &PathBuf) -> anyhow::Result<Vec<(String, String)>> {
    let lines =
        read_env_file(path).with_context(|| format!("failed to read {}", path.display()))?;

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
