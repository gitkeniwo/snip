use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::filesystem::atomic_write;

use super::palette::MAX_RECENT;

pub const STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionState {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_commands: Vec<String>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

impl SessionState {
    pub fn load() -> Self {
        state_path().map_or_else(|_| Self::default(), |path| Self::load_from(&path))
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        let Ok(mut state) = toml::from_str::<Self>(&text) else {
            return Self::default();
        };
        if state.schema_version > STATE_SCHEMA_VERSION {
            return Self::default();
        }
        state.recent_commands.truncate(MAX_RECENT);
        state
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&state_path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        let mut state = self.clone();
        state.schema_version = STATE_SCHEMA_VERSION;
        state.recent_commands.truncate(MAX_RECENT);
        let data = toml::to_string_pretty(&state)?;
        atomic_write(path, data.as_bytes())
    }
}

pub fn state_path() -> Result<PathBuf> {
    Ok(crate::config::config_path()?.with_file_name("state.toml"))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn round_trips_and_limits_recent_commands() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("state.toml");
        let state = SessionState {
            recent_commands: (0..MAX_RECENT + 3)
                .map(|index| format!("command.{index}"))
                .collect(),
            ..SessionState::default()
        };
        state.save_to(&path).unwrap();
        let loaded = SessionState::load_from(&path);
        assert_eq!(loaded.recent_commands.len(), MAX_RECENT);
        assert_eq!(loaded.recent_commands[0], "command.0");
        assert_eq!(loaded.recent_commands[MAX_RECENT - 1], "command.19");
    }

    #[test]
    fn ignores_unreadable_or_newer_state() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("state.toml");
        assert_eq!(
            SessionState::load_from(&path).recent_commands,
            Vec::<String>::new()
        );
        fs::write(&path, "not = [valid").unwrap();
        assert_eq!(
            SessionState::load_from(&path).recent_commands,
            Vec::<String>::new()
        );
        fs::write(
            &path,
            "schema_version = 99\nrecent_commands = ['git.push']\n",
        )
        .unwrap();
        assert_eq!(
            SessionState::load_from(&path).recent_commands,
            Vec::<String>::new()
        );
    }
}
