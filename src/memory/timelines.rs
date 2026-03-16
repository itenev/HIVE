use crate::models::scope::Scope;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSummary {
    pub narrative: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailySummary {
    pub narrative: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifetimeSummary {
    pub narrative: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimelineData {
    pub last_50_turns: Option<TurnSummary>,
    pub last_24_hours: Option<DailySummary>,
    pub lifetime: Option<LifetimeSummary>,
}

#[derive(Debug, Clone)]
pub struct TimelineStore {
    base_path: PathBuf,
}

impl TimelineStore {
    pub fn new(path: &std::path::Path) -> Self {
        Self {
            base_path: path.to_path_buf(),
        }
    }

    fn get_path(&self, scope: &Scope) -> PathBuf {
        let mut p = self.base_path.clone();
        match scope {
            Scope::Public { channel_id, user_id } => {
                p.push(format!("public_{}_{}", channel_id, user_id));
            }
            Scope::Private { user_id } => {
                p.push(format!("private_{}", user_id));
            }
        }
        let _ = std::fs::create_dir_all(&p);
        p.push("timelines.json");
        p
    }

    pub async fn read(&self, scope: &Scope) -> TimelineData {
        let path = self.get_path(scope);
        if let Ok(data) = tokio::fs::read_to_string(&path).await
            && let Ok(td) = serde_json::from_str(&data) {
                tracing::trace!("[MEMORY:Timelines] read: scope='{}' loaded", scope.to_key());
                return td;
            }
        tracing::trace!("[MEMORY:Timelines] read: scope='{}' returning defaults", scope.to_key());
        TimelineData::default()
    }

    pub async fn write(&self, scope: &Scope, data: &TimelineData) -> Result<(), String> {
        tracing::debug!("[MEMORY:Timelines] write: scope='{}' has_50turn={} has_daily={} has_lifetime={}",
            scope.to_key(), data.last_50_turns.is_some(), data.last_24_hours.is_some(), data.lifetime.is_some());
        let path = self.get_path(scope);
        let s = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
        tokio::fs::write(&path, s).await.map_err(|e| e.to_string())?;
        Ok(())
    }
}
