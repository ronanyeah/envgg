use anyhow::Context;
use indexmap::IndexSet;
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};
use std::path::PathBuf;

const TAG: &str = "envgg";

pub mod ui;

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
            let value = if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
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

    let var_names = lines.iter().map(|line| parse_env_line(line)).collect();

    Ok(var_names)
}

pub fn add_secret_to_keyring(key: &str, value: &str) -> anyhow::Result<()> {
    let entry = keyring_core::Entry::new(TAG, key)?;
    entry.set_password(value)?;
    Ok(())
}

pub fn delete_secret_from_keyring(key: &str) -> anyhow::Result<()> {
    let entry = keyring_core::Entry::new(TAG, key)?;
    entry.delete_credential()?;
    Ok(())
}

pub fn list_secret_labels() -> anyhow::Result<Vec<String>> {
    let search_params = HashMap::from([("service", TAG)]);

    let items = keyring_core::Entry::search(&search_params)?;

    let secret_names = items
        .iter()
        .map(|item| {
            let attributes = item.get_attributes()?;
            // Linux/Windows use "username", macOS uses "account"
            let name = attributes
                .get("username")
                .or_else(|| attributes.get("account"))
                .context("no key attribute")?;
            Ok::<_, anyhow::Error>(name.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(secret_names)
}

pub fn get_secret_from_keyring(target: &str) -> anyhow::Result<String> {
    let entry = keyring_core::Entry::new(TAG, target)?;
    let password = entry.get_password()?;
    Ok(password)
}
