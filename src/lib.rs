use anyhow::Context;
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

        // Case: KEY=$OTHER (unquoted) - alias for keyring lookup
        if value.starts_with('$') && !value.starts_with("$") && !value.starts_with("'") {
            let keyring_key = value[1..].trim().to_string();
            return EnvLine::Alias { key, keyring_key };
        } else {
            // Case: KEY=value - direct value assignment
            // Remove quotes if present
            let value = if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('"') && value.ends_with('"'))
            {
                value[1..value.len() - 1].to_string()
            } else {
                value
            };

            return EnvLine::Direct { key, value };
        }
    } else {
        // Case: KEY only (no =) - lookup from keyring
        let key = trimmed.to_string();
        return EnvLine::Lookup { key };
    }
}

pub fn get_env_var_names_from_file(path: &PathBuf) -> anyhow::Result<Vec<String>> {
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

    let var_names: Vec<String> = lines
        .iter()
        .filter_map(|line| match parse_env_line(line) {
            EnvLine::Comment => None,
            EnvLine::Alias { key, .. } => Some(key),
            EnvLine::Direct { key, .. } => Some(key),
            EnvLine::Lookup { key } => Some(key),
        })
        .collect();

    Ok(var_names)
}

pub async fn add_secret_to_keyring(key: &str, value: &str) -> anyhow::Result<()> {
    let entry = keyring_core::Entry::new(TAG, key)?;
    entry.set_password(value)?;
    Ok(())
}

pub async fn delete_secret_from_keyring(key: &str) -> anyhow::Result<()> {
    let entry = keyring_core::Entry::new(TAG, key)?;
    entry.delete_credential()?;
    Ok(())
}

pub async fn list_secrets() -> anyhow::Result<Vec<String>> {
    let search_params = HashMap::from([("service", TAG)]);

    let items = keyring_core::Entry::search(&search_params)?;

    let mut secret_names = items
        .iter()
        .map(|item| {
            let attributes = item.get_attributes()?;
            let name = attributes.get("username").context("no username")?;
            Ok::<_, anyhow::Error>(name.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;

    secret_names.sort();
    Ok(secret_names)
}

pub async fn get_secret_from_keyring(target: &str) -> anyhow::Result<String> {
    let entry = keyring_core::Entry::new(TAG, target)?;
    let password = entry.get_password()?;
    Ok(password)
}
