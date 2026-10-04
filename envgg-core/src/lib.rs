use anyhow::Context;
use chrono::{DateTime, Utc};
use indexmap::IndexSet;
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const TAG: &str = "envgg";

mod cli;

pub use cli::{OpenGui, run};

// Connected on first use, so commands that never touch a secret work on machines
// with no keyring service
fn ensure_keyring() -> anyhow::Result<()> {
    static CONNECTION: OnceLock<Result<(), String>> = OnceLock::new();
    CONNECTION
        .get_or_init(|| {
            connect_keyring()
                .context("failed to connect to the system keyring")
                .map_err(|e| format!("{e:#}"))
        })
        .clone()
        .map_err(anyhow::Error::msg)
}

fn connect_keyring() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    keyring_core::set_default_store(dbus_secret_service_keyring_store::Store::new()?);

    #[cfg(target_os = "macos")]
    keyring_core::set_default_store(apple_native_keyring_store::keychain::Store::new()?);

    #[cfg(target_os = "windows")]
    keyring_core::set_default_store(windows_native_keyring_store::store::Store::new()?);

    Ok(())
}

fn entry(key: &str) -> anyhow::Result<keyring_core::Entry> {
    ensure_keyring()?;
    Ok(keyring_core::Entry::new(TAG, key)?)
}

pub enum EnvLine {
    Comment,
    Alias { key: String, keyring_key: String },
    Direct { key: String, value: String },
    Lookup { key: String },
}

// TODO: should error for malformed entries
pub fn parse_env_line(line: &str) -> EnvLine {
    let trimmed = line.trim();

    // Skip empty lines and comments (lines starting with #)
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return EnvLine::Comment;
    }

    // Check for KEY=VALUE format
    if let Some(pos) = trimmed.find('=') {
        let key = trimmed[..pos].trim().to_string();
        let value = trimmed[pos + 1..].trim().to_string();

        // Case: KEY=$OTHER - alias for keyring lookup
        if let Some(val) = value.strip_prefix('$') {
            let keyring_key = val.trim().to_string();
            EnvLine::Alias { key, keyring_key }
        } else {
            // Case: KEY=value - direct value assignment
            // Remove quotes if present
            let value = if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\'')))
            {
                value[1..value.len() - 1].to_string()
            } else {
                value
            };

            EnvLine::Direct { key, value }
        }
    } else {
        // Case: KEY only (no =) - lookup from keyring
        let key = trimmed.to_string();
        EnvLine::Lookup { key }
    }
}

pub fn get_env_var_names_from_file(path: &PathBuf) -> anyhow::Result<IndexSet<String>> {
    let lines = read_env_file(path)?;

    let var_names: IndexSet<String> = lines
        .into_iter()
        .filter_map(|line| match line {
            EnvLine::Comment => None,
            EnvLine::Alias { key, .. } => Some(key),
            EnvLine::Direct { key, .. } => Some(key),
            EnvLine::Lookup { key } => Some(key),
        })
        .collect();

    Ok(var_names)
}

pub fn read_env_file(path: &PathBuf) -> anyhow::Result<Vec<EnvLine>> {
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

    let mut parsed = Vec::new();
    let mut iter = lines.into_iter();

    while let Some(mut logical) = iter.next() {
        // Join following lines while a quoted value is still open.
        // On EOF with an unterminated quote, use what we have.
        if let Some(quote) = open_quote(&logical) {
            for next in iter.by_ref() {
                logical.push('\n');
                logical.push_str(&next);
                if next.contains(quote) {
                    break;
                }
            }
        }
        parsed.push(parse_env_line(&logical));
    }

    Ok(parsed)
}

/// If the line is `KEY=<quote>...` with no closing quote, returns the quote char.
fn open_quote(line: &str) -> Option<char> {
    let trimmed = line.trim();
    if trimmed.starts_with('#') {
        return None;
    }
    let value = trimmed[trimmed.find('=')? + 1..].trim();
    let quote = value.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    (!value[1..].contains(quote)).then_some(quote)
}

#[derive(Clone)]
pub struct SecretInfo {
    pub name: String,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub description: String,
}

// Stored as unix seconds
fn stamp_secret(
    entry: &keyring_core::Entry,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
    description: &str,
) -> anyhow::Result<()> {
    let (created, updated) = (
        created.timestamp().to_string(),
        updated.timestamp().to_string(),
    );
    entry.update_attributes(&HashMap::from([
        ("created", created.as_str()),
        ("updated", updated.as_str()),
        ("description", description),
    ]))?;
    Ok(())
}

fn attribute<'a>(attributes: &'a HashMap<String, String>, key: &str) -> anyhow::Result<&'a String> {
    attributes
        .get(key)
        .with_context(|| format!("missing '{key}'"))
}

fn parse_time(attributes: &HashMap<String, String>, key: &str) -> anyhow::Result<DateTime<Utc>> {
    DateTime::from_timestamp(attribute(attributes, key)?.parse()?, 0)
        .with_context(|| format!("'{key}' out of range"))
}

pub fn add_secret_to_keyring(key: &str, value: &str) -> anyhow::Result<()> {
    let entry = entry(key)?;
    let now = Utc::now();
    // Read before overwriting, since set_password may reset attributes
    // A new secret has no entry yet, so no attributes
    let (created, description) = match entry.get_attributes() {
        Ok(attributes) => (
            parse_time(&attributes, "created")?,
            attribute(&attributes, "description")?.clone(),
        ),
        Err(_) => (now, String::new()),
    };
    entry.set_password(value)?;
    stamp_secret(&entry, created, now, &description)
}

pub fn delete_secret_from_keyring(key: &str) -> anyhow::Result<()> {
    let entry = entry(key)?;
    entry.delete_credential()?;
    Ok(())
}

/// SCREAMING_SNAKE_CASE: starts with a letter, then only A-Z, 0-9 or `_`
pub fn is_valid_env_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn search_attributes() -> anyhow::Result<Vec<HashMap<String, String>>> {
    ensure_keyring()?;
    keyring_core::Entry::search(&HashMap::from([("service", TAG)]))?
        .iter()
        .map(|item| Ok(item.get_attributes()?))
        .collect()
}

fn secret_name(attributes: &HashMap<String, String>) -> anyhow::Result<String> {
    // Linux/Windows use "username", macOS uses "account"
    let name = attributes
        .get("username")
        .or_else(|| attributes.get("account"))
        .context("no key attribute")?;
    Ok(name.clone())
}

/// All secrets with timestamps, sorted by name. Fails if any secret lacks them.
pub fn list_secrets() -> anyhow::Result<Vec<SecretInfo>> {
    let mut secrets = search_attributes()?
        .iter()
        .map(|attributes| {
            Ok(SecretInfo {
                name: secret_name(attributes)?,
                created: parse_time(attributes, "created")?,
                updated: parse_time(attributes, "updated")?,
                description: attribute(attributes, "description")?.clone(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    secrets.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(secrets)
}

/// Secret names only, sorted. Doesn't need timestamps.
pub fn list_secret_labels() -> anyhow::Result<Vec<String>> {
    let mut names = search_attributes()?
        .iter()
        .map(secret_name)
        .collect::<anyhow::Result<Vec<_>>>()?;

    names.sort();
    Ok(names)
}

pub fn get_secret_from_keyring(target: &str) -> anyhow::Result<String> {
    let password = entry(target)?.get_password()?;
    Ok(password)
}

// Quoted so the output reads back through `parse_env_line`, which has no escapes
fn quote_env_value(key: &str, value: &str) -> anyhow::Result<String> {
    if !value.contains('"') {
        Ok(format!("\"{value}\""))
    } else if !value.contains('\'') {
        Ok(format!("'{value}'"))
    } else {
        anyhow::bail!("'{key}' contains both quote kinds and can't be written to an env file")
    }
}

/// Writes every secret as `KEY="value"` to `path`, returning the count.
/// Refuses to overwrite an existing file unless `force` is set.
pub fn export_secrets(path: &Path, force: bool) -> anyhow::Result<usize> {
    let mut contents = String::new();
    let names = list_secret_labels()?;
    for name in &names {
        let value = get_secret_from_keyring(name)?;
        contents.push_str(&format!("{name}={}\n", quote_env_value(name, &value)?));
    }

    let mut options = fs::OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);

    let mut file = options.open(path).map_err(|e| match e.kind() {
        io::ErrorKind::AlreadyExists => {
            anyhow::anyhow!(
                "{} already exists, use --force to overwrite",
                path.display()
            )
        }
        _ => e.into(),
    })?;
    io::Write::write_all(&mut file, contents.as_bytes())?;

    Ok(names.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn read(contents: &str) -> Vec<EnvLine> {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("envgg-test-{}-{id}", std::process::id()));
        fs::write(&path, contents).unwrap();
        let out = read_env_file(&path).unwrap();
        fs::remove_file(&path).ok();
        out
    }

    fn assert_direct(line: &EnvLine, k: &str, v: &str) {
        match line {
            EnvLine::Direct { key, value } => {
                assert_eq!(key, k);
                assert_eq!(value, v);
            }
            _ => panic!("expected Direct for {k}"),
        }
    }

    #[test]
    fn multiline_value_is_joined() {
        let lines = read("A=\"l1\nl2\"\nB=x\n");
        assert_eq!(lines.len(), 2);
        assert_direct(&lines[0], "A", "l1\nl2");
        assert_direct(&lines[1], "B", "x");
    }

    #[test]
    fn single_quote_multiline() {
        let lines = read("A='l1\nl2'\nB=x\n");
        assert_eq!(lines.len(), 2);
        assert_direct(&lines[0], "A", "l1\nl2");
    }

    #[test]
    fn closed_quote_does_not_swallow_next_line() {
        let lines = read("A=\"x\"\nB=y\n");
        assert_eq!(lines.len(), 2);
        assert_direct(&lines[0], "A", "x");
        assert_direct(&lines[1], "B", "y");
    }

    #[test]
    fn other_quote_kind_inside_value() {
        let lines = read("A=\"it's\"\nB='say \"hi\"'\nC=z\n");
        assert_eq!(lines.len(), 3);
        assert_direct(&lines[0], "A", "it's");
        assert_direct(&lines[1], "B", "say \"hi\"");
        assert_direct(&lines[2], "C", "z");
    }

    #[test]
    fn comment_with_quote_is_ignored() {
        let lines = read("# KEY=\"abc\nB=y\n");
        assert_eq!(lines.len(), 2);
        assert!(matches!(lines[0], EnvLine::Comment));
        assert_direct(&lines[1], "B", "y");
    }

    #[test]
    fn mixed_file() {
        let lines = read("LOOK\nA=\"l1\nl2\"\nAL=$OTHER\n");
        assert_eq!(lines.len(), 3);
        assert!(matches!(&lines[0], EnvLine::Lookup { key } if key == "LOOK"));
        assert_direct(&lines[1], "A", "l1\nl2");
        assert!(
            matches!(&lines[2], EnvLine::Alias { key, keyring_key } if key == "AL" && keyring_key == "OTHER")
        );
    }

    #[test]
    fn blank_line_inside_value_is_kept() {
        let lines = read("A=\"l1\n\nl2\"\n");
        assert_eq!(lines.len(), 1);
        assert_direct(&lines[0], "A", "l1\n\nl2");
    }

    #[test]
    fn lone_quote_does_not_panic() {
        assert!(matches!(parse_env_line("K=\""), EnvLine::Direct { .. }));
    }

    #[test]
    fn unterminated_quote_reads_to_eof() {
        assert_eq!(read("A=\"l1\nl2\n").len(), 1);
    }
}
