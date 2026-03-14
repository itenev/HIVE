use std::path::PathBuf;
use tokio::fs;
use serde::{Deserialize, Serialize};
use crate::models::scope::Scope;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PreferencesData {
    pub name: Option<String>,
    pub hobbies: Vec<String>,
    pub topics_of_interest: Vec<String>,
    #[serde(default)]
    pub narrative_history: String,
    #[serde(default)]
    pub psychoanalysis: String,
}

impl PreferencesData {
    pub fn format_for_prompt(&self) -> String {
        let mut out = String::new();
        if let Some(ref n) = self.name {
            out.push_str(&format!("Name: {}\n", n));
        }
        if !self.hobbies.is_empty() {
            out.push_str(&format!("Hobbies: {}\n", self.hobbies.join(", ")));
        }
        if !self.topics_of_interest.is_empty() {
            out.push_str(&format!("Topics of Interest: {}\n", self.topics_of_interest.join(", ")));
        }
        if !self.narrative_history.is_empty() {
            out.push_str(&format!("Narrative History:\n{}\n", self.narrative_history));
        }
        if !self.psychoanalysis.is_empty() {
            out.push_str(&format!("Psychoanalysis:\n{}\n", self.psychoanalysis));
        }
        if out.is_empty() {
            "No preferences recorded.".to_string()
        } else {
            out
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreferenceStore {
    base_dir: PathBuf,
}

impl Default for PreferenceStore {
    fn default() -> Self {
        Self::new(None)
    }
}

impl PreferenceStore {
    pub fn new(base_dir: Option<PathBuf>) -> Self {
        #[cfg(test)]
        let default_dir = std::env::temp_dir().join(format!("hive_mem_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        #[cfg(not(test))]
        let default_dir = PathBuf::from("memory");

        Self {
            base_dir: base_dir.unwrap_or(default_dir),
        }
    }

    fn get_preferences_path(&self, scope: &Scope) -> PathBuf {
        let mut path = self.base_dir.clone();
        match scope {
            Scope::Public { channel_id, user_id } => {
                path.push(format!("public_{}", channel_id));
                path.push(user_id);
            }
            Scope::Private { user_id } => path.push(format!("private_{}", user_id)),
        }
        path.push("preferences.json");
        path
    }

    pub async fn read(&self, scope: &Scope) -> PreferencesData {
        let path = self.get_preferences_path(scope);
        if let Ok(data) = fs::read_to_string(&path).await {
            if let Ok(prefs) = serde_json::from_str(&data) {
                return prefs;
            }
        }
        PreferencesData::default()
    }

    pub async fn write(&self, scope: &Scope, prefs: &PreferencesData) -> std::io::Result<()> {
        let path = self.get_preferences_path(scope);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent).await;
        }
        let json = serde_json::to_string_pretty(prefs)?;
        fs::write(&path, json).await
    }
}
