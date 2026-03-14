use crate::models::tool::{ToolResult, ToolStatus};
use crate::memory::MemoryStore;
use tokio::sync::mpsc;
use std::sync::Arc;
use crate::agent::preferences::extract_tag;

pub async fn execute_operate_turing_grid(
    task_id: String,
    description: String,
    memory: Arc<MemoryStore>,
    telemetry_tx: Option<mpsc::Sender<String>>,
) -> ToolResult {
    let action = extract_tag(&description, "action:")
        .unwrap_or("read".to_string())
        .split_whitespace()
        .next()
        .unwrap_or("read")
        .to_lowercase();

    if let Some(ref tx) = telemetry_tx {
        let _ = tx
            .send(format!(
                "🚀 Turing Grid Drone executing action: `{}`\n",
                action
            ))
            .await;
    }

    let output;

    match action.as_str() {
        "move" => {
            let dx = extract_tag(&description, "dx:")
                .unwrap_or("0".to_string())
                .split_whitespace()
                .next()
                .unwrap_or("0")
                .parse::<i32>()
                .unwrap_or(0);
            let dy = extract_tag(&description, "dy:")
                .unwrap_or("0".to_string())
                .split_whitespace()
                .next()
                .unwrap_or("0")
                .parse::<i32>()
                .unwrap_or(0);
            let dz = extract_tag(&description, "dz:")
                .unwrap_or("0".to_string())
                .split_whitespace()
                .next()
                .unwrap_or("0")
                .parse::<i32>()
                .unwrap_or(0);

            let mut g = memory.turing_grid.lock().await;
            g.move_cursor(dx, dy, dz).await;
            let (x, y, z) = g.get_cursor();
            output = format!(
                "Moved Read/Write head. Current coordinates: ({}, {}, {})",
                x, y, z
            );
        }
        "scan" => {
            let radius = extract_tag(&description, "radius:")
                .unwrap_or("5".to_string())
                .split_whitespace()
                .next()
                .unwrap_or("5")
                .parse::<i32>()
                .unwrap_or(5);
            let g = memory.turing_grid.lock().await;
            let results = g.scan(radius);

            let mut out = String::new();
            for (coords, fmt) in results {
                out.push_str(&format!(
                    "* Cell ({}, {}, {}) [Format: {}]\n",
                    coords.0, coords.1, coords.2, fmt
                ));
            }
            if out.is_empty() {
                output = format!("No non-empty cells found within radius {}.", radius);
            } else {
                output = format!("--- Radar Scan (Radius {}): ---\n{}", radius, out);
            }
        }
        "write" => {
            let format_tag = extract_tag(&description, "format:").unwrap_or("text".to_string());
            let format_str = format_tag.split_whitespace().next().unwrap_or("text").to_string();
            let content = if let Some(idx) = description.find("content:") {
                description[idx + 8..].trim().to_string()
            } else {
                return ToolResult {
                    task_id,
                    output: "Error: No content provided for write.".into(),
                    tokens_used: 0,
                    status: ToolStatus::Failed("Missing field".into()),
                };
            };

            let mut g = memory.turing_grid.lock().await;
            let (x, y, z) = g.get_cursor();

            if let Err(e) = g.write_current(&format_str, &content).await {
                return ToolResult {
                    task_id,
                    output: format!("Failed to write: {}", e),
                    tokens_used: 0,
                    status: ToolStatus::Success,
                };
            }
            output = format!("Successfully wrote payload to cell ({}, {}, {}).", x, y, z);
        }
        "read" => {
            let g = memory.turing_grid.lock().await;
            let (x, y, z) = g.get_cursor();

            if let Some(cell) = g.read_current() {
                output = format!(
                    "Cell ({}, {}, {}) [Format: {}, Status: {}]:\n{}",
                    x, y, z, cell.format, cell.status, cell.content
                );
            } else {
                output = format!("Cell ({}, {}, {}) is empty.", x, y, z);
            }
        }
        "execute" => {
            let (format_str, content): (String, String) = {
                let mut g = memory.turing_grid.lock().await;
                let cell_info = g
                    .read_current()
                    .map(|cell| (cell.format.clone(), cell.content.clone()));

                match cell_info {
                    Some(info) => {
                        let _ = g.update_status("Running").await;
                        info
                    }
                    None => {
                        return ToolResult {
                            task_id,
                            output: "Error: Current cell is empty. Cannot execute.".into(),
                            tokens_used: 0,
                            status: ToolStatus::Success,
                        };
                    }
                }
            };

            let execute_result = memory.alu.execute_cell(&format_str, &content).await;

            let mut g = memory.turing_grid.lock().await;
            match execute_result {
                Ok(stdout) => {
                    let _ = g.update_status("Idle").await;
                    output = format!("Cell Executed Successfully.\nSTDOUT:\n{}", stdout);
                }
                Err(e) => {
                    let _ = g.update_status("Failed").await;
                    output = e;
                }
            }
        }
        _ => {
            return ToolResult {
                task_id,
                output: format!("Unknown action: {}", action),
                tokens_used: 0,
                status: ToolStatus::Failed("Invalid command".into()),
            };
        }
    }

    ToolResult {
        task_id,
        output,
        tokens_used: 0,
        status: ToolStatus::Success,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_operate_turing_grid() {
        let memory = Arc::new(MemoryStore::default());
        let (tx, _rx) = tokio::sync::mpsc::channel(10);

        // Test move
        let r1 = execute_operate_turing_grid(
            "id".into(),
            "action:move dx:1 dy:2 dz:3".into(),
            memory.clone(),
            Some(tx.clone()),
        )
        .await;
        assert!(r1.output.contains("(1, 2, 3)"));

        // Test write
        let r2 = execute_operate_turing_grid(
            "id".into(),
            "action:write format:text content:hello grid test".into(),
            memory.clone(),
            Some(tx.clone()),
        )
        .await;
        assert!(r2.output.contains("Successfully wrote"));

        // Test read
        let r3 = execute_operate_turing_grid(
            "id".into(),
            "action:read".into(),
            memory.clone(),
            Some(tx.clone()),
        )
        .await;
        assert!(r3.output.contains("hello grid test"));

        // Test write fail (no content tag)
        let r4 = execute_operate_turing_grid(
            "id".into(),
            "action:write format:text".into(),
            memory.clone(),
            Some(tx.clone()),
        )
        .await;
        assert!(r4.output.contains("No content provided"));

        // Test scan
        let r5 = execute_operate_turing_grid(
            "id".into(),
            "action:scan radius:5".into(),
            memory.clone(),
            Some(tx.clone()),
        )
        .await;
        assert!(r5.output.contains("Radar Scan") && r5.output.contains("(1, 2, 3)"));

        // Test unknown action
        let r6 = execute_operate_turing_grid(
            "id".into(),
            "action:invalid".into(),
            memory.clone(),
            Some(tx.clone()),
        )
        .await;
        assert!(r6.output.contains("Unknown action"));

        // Test execute on empty cell (move to empty cell first)
        let _ = execute_operate_turing_grid(
            "id".into(),
            "action:move dx:99 dy:99 dz:99".into(),
            memory.clone(),
            Some(tx.clone()),
        )
        .await;
        let r7 = execute_operate_turing_grid(
            "id".into(),
            "action:execute".into(),
            memory.clone(),
            Some(tx.clone()),
        )
        .await;
        assert!(r7.output.contains("Current cell is empty"));
    }
}
