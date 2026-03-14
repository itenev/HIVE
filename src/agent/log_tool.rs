use crate::models::tool::{ToolResult, ToolStatus};
use tokio::sync::mpsc;

pub async fn execute_read_logs(
    task_id: String,
    desc: String,
    telemetry_tx: Option<mpsc::Sender<String>>,
) -> ToolResult {
    if let Some(ref tx) = telemetry_tx {
        let _ = tx.send(format!("🧠 Native Log Reader Tool executing...\n")).await;
    }
    
    let mut lines_to_read = 50;
    if let Some(lines_str) = desc.split("lines:[").nth(1)
        && let Some(num_str) = lines_str.split("]").next()
            && let Ok(num) = num_str.parse::<usize>() {
                lines_to_read = num;
            }

    match tokio::fs::read_to_string("logs/hive.log").await {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let len = lines.len();
            let start = len.saturating_sub(lines_to_read);
            let tail = &lines[start..];
            let output = tail.join("\n");
            
            ToolResult {
                task_id,
                output: if output.is_empty() { 
                    "Log file is empty.".to_string() 
                } else { 
                    format!("{}\n\n[LOGS COMPLETE (Tailed {} lines)]", output, lines.len() - start) 
                },
                tokens_used: 0,
                status: ToolStatus::Success,
            }
        }
        Err(e) => {
            ToolResult {
                task_id,
                output: format!("Failed to read logs: {}", e),
                tokens_used: 0,
                status: ToolStatus::Failed(e.to_string()),
            }
        }
    }
}
