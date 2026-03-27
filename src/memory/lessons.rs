use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use crate::models::scope::Scope;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lesson {
    /// Unique ID for deduplication across mesh peers.
    #[serde(default = "default_lesson_id")]
    pub id: String,
    pub text: String,
    pub keywords: Vec<String>,
    pub confidence: f32,
    /// PeerId of the instance that learned this. "local" for locally-created lessons.
    #[serde(default = "default_origin")]
    pub origin: String,
    /// RFC3339 timestamp of when this lesson was learned.
    #[serde(default = "default_timestamp")]
    pub learned_at: String,
}

fn default_lesson_id() -> String { Uuid::new_v4().to_string() }
fn default_origin() -> String { "local".to_string() }
fn default_timestamp() -> String { chrono::Utc::now().to_rfc3339() }

#[derive(Debug, Clone)]
pub struct LessonsManager {
    base_dir: PathBuf,
}

impl LessonsManager {
    pub fn new(base_dir: Option<PathBuf>) -> Self {
        #[cfg(test)]
        let default_dir = std::env::temp_dir().join(format!(
            "hive_mem_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        #[cfg(not(test))]
        let default_dir = PathBuf::from("memory");

        Self {
            base_dir: base_dir.unwrap_or(default_dir),
        }
    }

    fn get_lessons_path(&self, scope: &Scope) -> PathBuf {
        let mut path = self.base_dir.clone();
        match scope {
            Scope::Public { channel_id, user_id } => {
                path.push(format!("public_{}", channel_id));
                path.push(user_id);
            }
            Scope::Private { user_id } => path.push(format!("private_{}", user_id)),
        }
        path.push("lessons.jsonl");
        path
    }

    pub async fn add_lesson(&self, scope: &Scope, lesson: &Lesson) -> std::io::Result<()> {
        tracing::debug!("[MEMORY:Lessons] add_lesson: scope='{}' keywords={:?} confidence={}", scope.to_key(), lesson.keywords, lesson.confidence);
        let path = self.get_lessons_path(scope);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent).await;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;

        let json = serde_json::to_string(lesson)?;
        file.write_all(format!("{}\n", json).as_bytes()).await?;
        file.sync_all().await?;
        Ok(())
    }

    pub async fn read_lessons(&self, scope: &Scope) -> Vec<Lesson> {
        let path = self.get_lessons_path(scope);
        let mut lessons = Vec::new();

        if let Ok(content) = fs::read_to_string(&path).await {
            for line in content.lines() {
                if let Ok(lesson) = serde_json::from_str::<Lesson>(line) {
                    lessons.push(lesson);
                }
            }
        }
        tracing::debug!("[MEMORY:Lessons] read_lessons: scope='{}' count={}", scope.to_key(), lessons.len());
        lessons
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lessons_manager() {
        let test_dir = std::env::temp_dir().join(format!("hive_lessons_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let manager = LessonsManager::new(Some(test_dir.clone()));

        let pub_scope = Scope::Public { channel_id: "c1".into(), user_id: "u1".into() };
        let priv_scope = Scope::Private { user_id: "u2".into() };

        let lesson1 = Lesson {
            id: Uuid::new_v4().to_string(),
            text: "Fire is hot".into(),
            keywords: vec!["fire".into(), "hot".into()],
            confidence: 0.9,
            origin: "local".into(),
            learned_at: chrono::Utc::now().to_rfc3339(),
        };

        let lesson2 = Lesson {
            id: Uuid::new_v4().to_string(),
            text: "Water is wet".into(),
            keywords: vec!["water".into()],
            confidence: 0.99,
            origin: "local".into(),
            learned_at: chrono::Utc::now().to_rfc3339(),
        };

        // Write lesson to public scope
        manager.add_lesson(&pub_scope, &lesson1).await.unwrap();
        // Write lesson to private scope
        manager.add_lesson(&priv_scope, &lesson2).await.unwrap();

        // Read back
        let pub_lessons = manager.read_lessons(&pub_scope).await;
        assert_eq!(pub_lessons.len(), 1);
        assert_eq!(pub_lessons[0].text, "Fire is hot");

        let priv_lessons = manager.read_lessons(&priv_scope).await;
        assert_eq!(priv_lessons.len(), 1);
        assert_eq!(priv_lessons[0].text, "Water is wet");

        // Read non-existent scope
        let empty_scope = Scope::Public { channel_id: "empty".into(), user_id: "empty".into() };
        let no_lessons = manager.read_lessons(&empty_scope).await;
        assert_eq!(no_lessons.len(), 0);

        // Test default creation (just ensure it doesn't crash)
        let _def_manager = LessonsManager::new(None);
        
        // Clean up
        let _ = tokio::fs::remove_dir_all(&test_dir).await;
    }
}
