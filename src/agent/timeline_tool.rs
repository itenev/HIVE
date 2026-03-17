use crate::models::tool::{ToolResult, ToolStatus};
use crate::memory::MemoryStore;
use crate::models::scope::Scope;
use crate::agent::preferences::extract_tag;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::io::{AsyncBufReadExt, BufReader};

pub async fn execute_search_timeline(
    task_id: String,
    description: String,
    _memory: Arc<MemoryStore>,
    telemetry_tx: Option<mpsc::Sender<String>>,
    current_scope: &Scope,
) -> ToolResult {
    if let Some(ref tx) = telemetry_tx {
        let _ = tx.send("🧠 Timeline Drone executing...\n".to_string()).await;
    }
    tracing::debug!("[AGENT:timeline] ▶ task_id={}", task_id);

    let query = extract_tag(&description, "query:").unwrap_or_default().to_lowercase();
    let limit_str = extract_tag(&description, "limit:").unwrap_or("20".to_string());
    let limit: usize = limit_str.parse().unwrap_or(20);
    let scope_override = extract_tag(&description, "scope:").unwrap_or_default().to_lowercase();

    if query.is_empty() {
        return ToolResult { 
            task_id, 
            output: "Error: Missing 'query:' field.".to_string(), 
            tokens_used: 0, 
            status: ToolStatus::Failed("Missing field".into()) 
        };
    }

    // Determine which timeline directories to search:
    // - scope:[all_public] → search all public_* directories
    // - scope:[<channel_id>] → search a specific channel  
    // - no scope → search current channel only
    let dirs_to_search: Vec<std::path::PathBuf> = if scope_override == "all_public" {
        // Sweep all public timelines
        let mut dirs = Vec::new();
        if let Ok(mut entries) = tokio::fs::read_dir("memory").await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("public_") && entry.path().is_dir() {
                    dirs.push(entry.path());
                }
            }
        }
        if dirs.is_empty() {
            return ToolResult {
                task_id,
                output: "No public timelines exist yet.".to_string(),
                tokens_used: 0,
                status: ToolStatus::Success,
            };
        }
        dirs
    } else if !scope_override.is_empty() {
        // Cross-channel: search a specific channel by ID
        vec![std::path::PathBuf::from(format!("memory/public_{}", scope_override))]
    } else {
        // Default: current scope only
        let dir_path = match current_scope {
            Scope::Public { channel_id, .. } => std::path::PathBuf::from(format!("memory/public_{}", channel_id)),
            Scope::Private { user_id } => std::path::PathBuf::from(format!("memory/private_{}", user_id)),
        };
        vec![dir_path]
    };

    // Search across all targeted directories
    let mut results = Vec::new();
    let mut searched_count = 0;

    for dir_path in &dirs_to_search {
        let timeline_path = dir_path.join("timeline.jsonl");
        if let Ok(file) = tokio::fs::File::open(&timeline_path).await {
            searched_count += 1;
            let reader = BufReader::new(file);
            let mut lines = reader.lines();
            let mut all_lines = Vec::new();
            while let Ok(Some(line)) = lines.next_line().await {
                all_lines.push(line);
            }
            
            // Extract channel name for multi-channel context
            let channel_label = dir_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            for line in all_lines.iter().rev() {
                if line.to_lowercase().contains(&query)
                    && let Ok(json) = serde_json::from_str::<serde_json::Value>(line)
                        && let (Some(author), Some(content)) = (json["author_name"].as_str(), json["content"].as_str()) {
                            let prefix = if dirs_to_search.len() > 1 {
                                format!("[{}] {}", channel_label, author)
                            } else {
                                author.to_string()
                            };
                            results.push(format!("{}: {}", prefix, content));
                            if results.len() >= limit {
                                break;
                            }
                        }
            }

            if results.len() >= limit {
                break;
            }
        }
    }

    if searched_count == 0 {
        return ToolResult {
            task_id,
            output: "No long-term timeline exists for this scope yet.".to_string(),
            tokens_used: 0,
            status: ToolStatus::Success,
        };
    }

    results.reverse(); // Chronological order (oldest match to newest match)

    if results.is_empty() {
        ToolResult {
            task_id,
            output: format!("No matches found for '{}' across {} timeline(s) searched.", query, searched_count),
            tokens_used: 0,
            status: ToolStatus::Success,
        }
    } else {
        ToolResult {
            task_id,
            output: format!("Timeline Search Results for '{}' ({} timeline(s) searched):\n\n{}", query, searched_count, results.join("\n\n")),
            tokens_used: 0,
            status: ToolStatus::Success,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_execute_search_timeline() {
        let mem = Arc::new(MemoryStore::default());
        let scope = Scope::Private { user_id: "test_tl_user".to_string() };

        // Ensure missing query fails
        let res = execute_search_timeline("1".into(), "limit:[5]".into(), mem.clone(), None, &scope).await;
        assert_eq!(res.status, ToolStatus::Failed("Missing field".into()));

        // Empty timeline search should yield success but "no timeline"
        let res = execute_search_timeline("1".into(), "query:[apple]".into(), mem.clone(), None, &scope).await;
        assert!(res.output.contains("No long-term timeline exists"));

        // Setup some fake timeline data
        let dir = std::path::PathBuf::from("memory/private_test_tl_user");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let file_path = dir.join("timeline.jsonl");
        
        // Write some JSONL lines
        let ev1 = serde_json::json!({"author_name": "User", "content": "I like apple pie."}).to_string();
        let ev2 = serde_json::json!({"author_name": "Apis", "content": "I prefer banana bread."}).to_string();
        let ev3 = serde_json::json!({"author_name": "User", "content": "Another apple reference."}).to_string();
        
        tokio::fs::write(&file_path, format!("{}\n{}\n{}\n", ev1, ev2, ev3)).await.unwrap();

        // Search for 'apple' with standard limit
        let res = execute_search_timeline("1".into(), "query:[apple]".into(), mem.clone(), None, &scope).await;
        assert_eq!(res.status, ToolStatus::Success);
        assert!(res.output.contains("User: I like apple pie."));
        assert!(res.output.contains("User: Another apple reference."));
        assert!(!res.output.contains("banana"));

        // Search for 'apple' with limit 1
        let res = execute_search_timeline("2".into(), "query:[apple] limit:[1]".into(), mem.clone(), None, &scope).await;
        assert_eq!(res.status, ToolStatus::Success);
        assert!(!res.output.contains("User: I like apple pie.")); // Older one should be dropped
        assert!(res.output.contains("User: Another apple reference.")); // Newer one is kept

        // Cleanup
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn test_cross_channel_search() {
        let mem = Arc::new(MemoryStore::default());
        let scope = Scope::Public { channel_id: "channel_A".to_string(), user_id: "user1".to_string() };

        // Create a timeline in a DIFFERENT channel (channel_B)
        let dir_b = std::path::PathBuf::from("memory/public_channel_B");
        tokio::fs::create_dir_all(&dir_b).await.unwrap();
        let ev = serde_json::json!({"author_name": "Zenzic", "content": "Orthogonal inversion mirrored"}).to_string();
        tokio::fs::write(dir_b.join("timeline.jsonl"), format!("{}\n", ev)).await.unwrap();

        // Searching from channel_A for Zenzic should fail (default scope)
        let res = execute_search_timeline("1".into(), "query:[zenzic]".into(), mem.clone(), None, &scope).await;
        assert!(res.output.contains("No long-term timeline exists"), "Default scope should not see other channels");

        // With scope:[channel_B], should find it
        let res = execute_search_timeline("2".into(), "query:[zenzic] scope:[channel_B]".into(), mem.clone(), None, &scope).await;
        assert_eq!(res.status, ToolStatus::Success);
        assert!(res.output.contains("Zenzic"), "Cross-channel search should find Zenzic, got: {}", res.output);

        // Cleanup
        let _ = tokio::fs::remove_dir_all(dir_b).await;
    }
}
