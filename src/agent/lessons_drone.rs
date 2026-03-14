use crate::models::tool::{ToolResult, ToolStatus};
use crate::memory::MemoryStore;
use crate::models::scope::Scope;
use std::sync::Arc;
use tokio::sync::mpsc;

fn extract_tag(desc: &str, tag: &str) -> Option<String> {
    if let Some(start_idx) = desc.find(tag) {
        let after_tag = &desc[start_idx + tag.len()..];
        if after_tag.starts_with('[') {
            if let Some(end_idx) = after_tag.find(']') {
                return Some(after_tag[1..end_idx].trim().to_string());
            }
        }
    }
    None
}

pub async fn execute_store_lesson(
    task_id: String,
    description: String,
    memory: Arc<MemoryStore>,
    telemetry_tx: Option<mpsc::Sender<String>>,
) -> ToolResult {
    if let Some(ref tx) = telemetry_tx {
        let _ = tx.send("🧠 Store Lesson Drone executing...\n".to_string()).await;
    }

    let lesson_text = extract_tag(&description, "lesson:").unwrap_or("".to_string());
    let keywords_str = extract_tag(&description, "keywords:").unwrap_or("".to_string());
    let confidence_str = extract_tag(&description, "confidence:").unwrap_or("1.0".to_string());

    if lesson_text.is_empty() {
        return ToolResult { 
            task_id, 
            output: "Error: Missing 'lesson:' field.".to_string(), 
            tokens_used: 0, 
            status: ToolStatus::Failed("Missing field".into()) 
        };
    }

    let confidence: f32 = confidence_str.parse().unwrap_or(1.0f32).clamp(0.0f32, 1.0f32);
    let keywords: Vec<String> = keywords_str.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let scope_str = extract_tag(&description, "scope:").unwrap_or("public_general".to_string());
    let scope = if scope_str.starts_with("public_") {
        Scope::Public { channel_id: scope_str.replace("public_", ""), user_id: "system".into() }
    } else {
        Scope::Private { user_id: scope_str.replace("private_", "") }
    };

    let lesson = crate::memory::lessons::Lesson {
        text: lesson_text,
        keywords,
        confidence,
    };

    match memory.lessons.add_lesson(&scope, &lesson).await {
        Ok(_) => ToolResult { task_id, output: "Lesson stored successfully.".to_string(), tokens_used: 0, status: ToolStatus::Success },
        Err(e) => ToolResult { task_id, output: format!("Failed to store lesson: {}", e), tokens_used: 0, status: ToolStatus::Failed(e.to_string()) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lessons_drone_execute() {
        let mem = Arc::new(MemoryStore::default());

        // Test missing lesson
        let res = execute_store_lesson("1".into(), "keywords:[fire, burn]".into(), mem.clone(), None).await;
        assert_eq!(res.status, ToolStatus::Failed("Missing field".into()));

        // Test successful public store
        let res = execute_store_lesson(
            "2".into(),
            "lesson:[Fire is hot] keywords:[fire, hot] confidence:[0.9] scope:[public_general]".into(),
            mem.clone(),
            None
        ).await;
        assert_eq!(res.status, ToolStatus::Success);

        // Test successful private store (malformed confidence fallback to 1.0)
        let res = execute_store_lesson(
            "3".into(),
            "lesson:[Water is wet] keywords:[water] confidence:[high] scope:[private_userX]".into(),
            mem.clone(),
            None
        ).await;
        assert_eq!(res.status, ToolStatus::Success);

        // Verify write was successful
        let pub_lessons = mem.lessons.read_lessons(&Scope::Public { channel_id: "general".into(), user_id: "system".into() }).await;
        assert_eq!(pub_lessons.len(), 1);
        assert_eq!(pub_lessons[0].confidence, 0.9);

        let priv_lessons = mem.lessons.read_lessons(&Scope::Private { user_id: "userX".into() }).await;
        assert_eq!(priv_lessons.len(), 1);
        assert_eq!(priv_lessons[0].confidence, 1.0); // Fallback parse
    }
}
