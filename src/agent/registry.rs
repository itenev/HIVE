use std::sync::Arc;
use tokio::sync::mpsc;
use crate::models::tool::{ToolResult, ToolStatus};
use crate::models::scope::Scope;
use crate::memory::MemoryStore;
use crate::providers::Provider;

pub fn dispatch_native_tool(
    task: &crate::agent::planner::AgentTask,
    context: &str,
    scope: &Scope,
    telemetry_tx: Option<mpsc::Sender<String>>,
    memory: Arc<MemoryStore>,
    provider: Arc<dyn Provider>,
    outreach_gate: Option<Arc<crate::engine::outreach::OutreachGate>>,
    inbox: Option<Arc<crate::engine::inbox::InboxManager>>,
    drives: Option<Arc<crate::engine::drives::DriveSystem>>,
) -> Option<tokio::task::JoinHandle<ToolResult>> {
    let task_id = task.task_id.clone();
    let desc = task.description.clone();
    let tx_clone = telemetry_tx.clone();
    let tool_type = task.tool_type.as_str();

    if tool_type == "channel_reader" {
        let mem_clone = memory.clone();
        let handle = tokio::spawn(async move {
            if let Some(ref tx) = tx_clone {
                let _ = tx.send(format!("🧠 Native Channel Reader Tool executing...\n")).await;
            }
            let target_id = desc.split_whitespace().last().unwrap_or(&"").to_string();
            let pub_scope = Scope::Public { channel_id: target_id.clone(), user_id: "system".into() };

            let output = if let Ok(timeline_data) = mem_clone.timeline.read_timeline(&pub_scope).await {
                String::from_utf8_lossy(&timeline_data).to_string()
            } else {
                "Failed to read timeline for channel.".to_string()
            };
            
            ToolResult {
                task_id,
                output,
                tokens_used: 0,
                status: ToolStatus::Success,
            }
        });
        return Some(handle);
    } 
    
    if tool_type == "outreach" {
        let handle = tokio::spawn(crate::agent::outreach::execute_outreach(
            task_id, desc, outreach_gate, inbox, drives, tx_clone,
        ));
        return Some(handle);
    } 
    
    if tool_type == "codebase_list" {
        let handle = tokio::spawn(async move {
            if let Some(ref tx) = tx_clone {
                let _ = tx.send(format!("🧠 Native Codebase List Tool executing...\n")).await;
            }
            let output = match std::process::Command::new("find").arg("src").arg("-type").arg("f").output() {
                Ok(res) => String::from_utf8_lossy(&res.stdout).to_string(),
                Err(e) => format!("Failed to list codebase: {}", e),
            };
            ToolResult { task_id, output, tokens_used: 0, status: ToolStatus::Success }
        });
        return Some(handle);
    } 
    
    if tool_type == "codebase_read" {
        let handle = tokio::spawn(async move {
            crate::agent::file_reader::execute_file_reader(task_id, desc, tx_clone).await
        });
        return Some(handle);
    } 
    
    if tool_type == "web_search" || tool_type == "researcher" {
        let handle = tokio::spawn(async move {
            crate::agent::web_drone::execute_web_search(task_id, desc, tx_clone).await
        });
        return Some(handle);
    } 
    
    if tool_type == "generate_image" {
        let ctx_str = context.to_string();
        if ctx_str.contains("[ATTACH_IMAGE]") {
            if let Some(tx) = tx_clone.clone() {
                let _ = tokio::spawn(async move {
                    let _ = tx.send("⚠️ Blocked duplicate image generation attempt.\n".into()).await;
                });
            }
            let failure_result = ToolResult {
                task_id,
                output: "FATAL SYSTEM ERROR: YOU ALREADY GENERATED AN IMAGE IN THIS TURN. YOU ARE FORBIDDEN FROM GENERATING MULTIPLE IMAGES PER USER REQUEST. STOP USING TOOLS AND REPLY TO THE USER IMMEDIATELY.".to_string(),
                tokens_used: 0,
                status: ToolStatus::Failed("Blocked Duplicate Render".to_string())
            };
            return Some(tokio::spawn(async move { failure_result }));
        } else {
            return Some(tokio::spawn(crate::agent::image_drone::execute_generate_image(task_id, desc, tx_clone)));
        }
    } 
    
    if tool_type == "voice_synthesizer" {
        let handle = tokio::spawn(async move {
            crate::agent::tts_drone::execute_voice_synthesizer(task_id, desc, tx_clone).await
        });
        return Some(handle);
    } 
    
    if tool_type == "operate_turing_grid" {
        let mem_clone = memory.clone();
        let handle = tokio::spawn(async move {
            crate::agent::turing_drone::execute_operate_turing_grid(task_id, desc, mem_clone, tx_clone).await
        });
        return Some(handle);
    } 
    
    if tool_type == "file_writer" {
        let handle = tokio::spawn(async move {
            crate::agent::file_writer::execute_file_writer(task_id, desc, None, tx_clone).await
        });
        return Some(handle);
    } 
    
    if tool_type == "read_logs" {
        let handle = tokio::spawn(async move {
            if let Some(ref tx) = tx_clone {
                let _ = tx.send(format!("🧠 Native Log Reader Tool executing...\n")).await;
            }
            
            let mut lines_to_read = 50;
            if let Some(lines_str) = desc.split("lines:[").nth(1) {
                if let Some(num_str) = lines_str.split("]").next() {
                    if let Ok(num) = num_str.parse::<usize>() {
                        lines_to_read = num;
                    }
                }
            }

            match tokio::fs::read_to_string("logs/hive.log").await {
                Ok(content) => {
                    let lines: Vec<&str> = content.lines().collect();
                    let len = lines.len();
                    let start = if len > lines_to_read { len - lines_to_read } else { 0 };
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
        });
        return Some(handle);
    } 
    if tool_type == "run_bash_command" {
        let mem_clone = memory.clone();
        let name_clone = tool_type.to_string();
        let handle = tokio::spawn(async move {
            if let Some(ref tx) = tx_clone {
                let _ = tx.send(format!("⚙️ Native {} executing...\n", name_clone)).await;
            }
            let res = mem_clone.alu.execute_cell("bash", &desc).await;
            match res {
                Ok(output) => ToolResult {
                    task_id,
                    output: if output.is_empty() { "Command succeeded with no output.".into() } else { output },
                    tokens_used: 0,
                    status: ToolStatus::Success,
                },
                Err(e) => ToolResult {
                    task_id,
                    output: e.clone(),
                    tokens_used: 0,
                    status: ToolStatus::Failed(e),
                }
            }
        });
        return Some(handle);
    }
    if tool_type == "process_manager" {
        let handle = tokio::spawn(async move {
            crate::agent::process_manager::execute_process_manager(task_id, desc, tx_clone).await
        });
        return Some(handle);
    }
    
    if tool_type == "file_system_operator" {
        let handle = tokio::spawn(async move {
            if let Some(ref tx) = tx_clone {
                let _ = tx.send("📁 Native File System Operator executing...\n".to_string()).await;
            }
            
            let action = crate::agent::preferences::extract_tag(&desc, "action:").unwrap_or_default();
            let path_str = crate::agent::preferences::extract_tag(&desc, "path:").unwrap_or_default();
            
            if action.is_empty() || path_str.is_empty() {
                return ToolResult {
                    task_id,
                    output: "Error: Missing action:[...] or path:[...]".into(),
                    tokens_used: 0,
                    status: ToolStatus::Failed("Invalid Args".into()),
                };
            }
            
            let path = std::path::Path::new(&path_str);
            let final_output;
            let mut is_err = false;
            
            match action.as_str() {
                "write" => {
                    let content = if let Some(idx) = desc.find("content:[") {
                        let mut content_body = desc[idx + 9..].to_string();
                        if content_body.ends_with(']') {
                            content_body.pop();
                        }
                        content_body
                    } else {
                        "".to_string()
                    };
                    
                    if let Some(parent) = path.parent() {
                        let _ = tokio::fs::create_dir_all(parent).await;
                    }
                    if let Err(e) = tokio::fs::write(&path, content).await {
                        final_output = format!("Failed to write: {}", e);
                        is_err = true;
                    } else {
                        final_output = format!("Successfully wrote to {}", path_str);
                    }
                }
                "append" => {
                    let content = if let Some(idx) = desc.find("content:[") {
                        let mut content_body = desc[idx + 9..].to_string();
                        if content_body.ends_with(']') {
                            content_body.pop();
                        }
                        content_body
                    } else {
                        "".to_string()
                    };
                    
                    use tokio::io::AsyncWriteExt;
                    match tokio::fs::OpenOptions::new().create(true).append(true).open(&path).await {
                        Ok(mut file) => {
                            if let Err(e) = file.write_all(content.as_bytes()).await {
                                final_output = format!("Failed to append: {}", e);
                                is_err = true;
                            } else {
                                final_output = format!("Successfully appended to {}", path_str);
                            }
                        }
                        Err(e) => {
                            final_output = format!("Failed to open for append: {}", e);
                            is_err = true;
                        }
                    }
                }
                "delete" => {
                    if path.is_file() {
                        if let Err(e) = tokio::fs::remove_file(&path).await {
                            final_output = format!("Failed to delete file: {}", e);
                            is_err = true;
                        } else {
                            final_output = format!("Successfully deleted file {}", path_str);
                        }
                    } else if path.is_dir() {
                        if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                            final_output = format!("Failed to delete directory: {}", e);
                            is_err = true;
                        } else {
                            final_output = format!("Successfully deleted directory {}", path_str);
                        }
                    } else {
                        // If path doesn't exist, we still consider it a success if the user's goal was to make it not exist.
                        final_output = format!("Successfully verified {} does not exist", path_str);
                    }
                }
                _ => {
                    final_output = format!("Unknown action: {}", action);
                    is_err = true;
                }
            }
            
            ToolResult {
                task_id,
                output: final_output.clone(),
                tokens_used: 0,
                status: if is_err { ToolStatus::Failed(final_output) } else { ToolStatus::Success },
            }
        });
        return Some(handle);
    } 
    
    if tool_type == "autonomy_activity" {
        let handle = tokio::spawn(async move {
            if let Some(ref tx) = tx_clone {
                let _ = tx.send("🐝 Autonomy Activity Tool executing...\n".to_string()).await;
            }

            let path = std::path::Path::new("memory/autonomy/activity.jsonl");
            let content = match tokio::fs::read_to_string(path).await {
                Ok(c) => c,
                Err(_) => {
                    return ToolResult {
                        task_id,
                        output: "No autonomous activity recorded yet. The autonomy log is empty.".to_string(),
                        tokens_used: 0,
                        status: ToolStatus::Success,
                    };
                }
            };

            let entries: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

            if desc.contains("action:[summary]") {
                let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
                let mut session_count = 0;
                let mut total_turns = 0;
                let mut tools_used = std::collections::HashSet::new();
                let mut summaries = Vec::new();

                for line in &entries {
                    if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                        if let Some(ts) = entry.get("timestamp").and_then(|v| v.as_str()) {
                            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                                if dt < cutoff { continue; }
                            }
                        }
                        session_count += 1;
                        total_turns += entry.get("turn_count").and_then(|v| v.as_u64()).unwrap_or(0);
                        if let Some(tools) = entry.get("tools_used").and_then(|v| v.as_array()) {
                            for t in tools {
                                if let Some(s) = t.as_str() { tools_used.insert(s.to_string()); }
                            }
                        }
                        if let Some(summary) = entry.get("summary").and_then(|v| v.as_str()) {
                            summaries.push(summary.to_string());
                        }
                    }
                }

                let output = format!(
                    "📊 **Autonomy Summary (Last 24h)**\n• Sessions: {}\n• Total turns used: {}\n• Tools exercised: {}\n\n**Session Highlights:**\n{}",
                    session_count,
                    total_turns,
                    tools_used.into_iter().collect::<Vec<_>>().join(", "),
                    summaries.iter().enumerate().map(|(i, s)| format!("{}. {}", i + 1, s)).collect::<Vec<_>>().join("\n")
                );

                ToolResult {
                    task_id,
                    output: if session_count == 0 { "No autonomous activity in the last 24 hours.".to_string() } else { output },
                    tokens_used: 0,
                    status: ToolStatus::Success,
                }
            } else {
                let mut count = 10usize;
                if let Some(count_str) = desc.split("count:[").nth(1) {
                    if let Some(num_str) = count_str.split(']').next() {
                        if let Ok(n) = num_str.parse::<usize>() { count = n; }
                    }
                }

                let start = if entries.len() > count { entries.len() - count } else { 0 };
                let recent = &entries[start..];
                let output = recent.join("\n");

                ToolResult {
                    task_id,
                    output: if output.is_empty() { "No autonomous activity recorded yet.".to_string() } else { output },
                    tokens_used: 0,
                    status: ToolStatus::Success,
                }
            }
        });
        return Some(handle);
    } 
    
    if tool_type == "review_reasoning" {
        let mem_clone = memory.clone();
        let scope_clone = scope.clone();
        let handle = tokio::spawn(async move {
            if let Some(ref tx) = tx_clone {
                let _ = tx.send(format!("🧠 Native Reasoning Review Tool executing...\n")).await;
            }

            let mut turns_ago = 5;
            if let Some(turns_str) = desc.split("turns_ago:[").nth(1) {
                if let Some(num_str) = turns_str.split("]").next() {
                    if let Ok(num) = num_str.parse::<usize>() {
                        turns_ago = num;
                    }
                }
            }

            let history = mem_clone.working.get_history(&scope_clone).await;
            let mut extracted = Vec::new();
            for event in history.iter().rev() {
                if event.author_name == "Apis (Internal Timeline)" {
                    extracted.push(event.content.clone());
                }
            }

            if extracted.is_empty() {
                return ToolResult {
                    task_id,
                    output: "No reasoning traces found in active memory.".to_string(),
                    tokens_used: 0,
                    status: ToolStatus::Success,
                };
            }

            let start_idx = if turns_ago >= extracted.len() { extracted.len() - 1 } else { turns_ago };
            
            let slice = if start_idx + 5 <= extracted.len() {
                &extracted[start_idx..start_idx + 5]
            } else {
                &extracted[start_idx..]
            };

            let mut out = String::new();
            for (i, trace) in slice.iter().enumerate() {
                out.push_str(&format!("--- TRACE {} TURNS AGO ---\n{}\n\n", start_idx + i, trace));
            }

            ToolResult {
                task_id,
                output: out,
                tokens_used: 0,
                status: ToolStatus::Success,
            }
        });
        return Some(handle);
    } 
    
    if tool_type == "read_attachment" {
        let handle = tokio::spawn(async move {
            if let Some(ref tx) = tx_clone {
                let _ = tx.send("📎 Fetching attachment (in-memory, no disk write)...\n".to_string()).await;
            }

            let url = crate::agent::preferences::extract_tag(&desc, "url:")
                .unwrap_or_else(|| {
                    desc.split_whitespace()
                        .find(|s| s.starts_with("http"))
                        .map(|s| s.trim_matches(|c| c == '\'' || c == '"' || c == '`' || c == '(' || c == ')' || c == ']').to_string())
                        .unwrap_or_default()
                });

            if url.is_empty() || !url.starts_with("http") {
                return ToolResult {
                    task_id,
                    output: "Error: No valid URL provided. Use url:[https://cdn.discordapp.com/...]".into(),
                    tokens_used: 0,
                    status: ToolStatus::Failed("Missing or invalid URL".into()),
                };
            }

            let allowed_domains = ["cdn.discordapp.com", "media.discordapp.net"];
            let is_allowed = allowed_domains.iter().any(|d| url.contains(d));
            if !is_allowed {
                return ToolResult {
                    task_id,
                    output: "Access Denied: read_attachment only accepts Discord CDN URLs.".into(),
                    tokens_used: 0,
                    status: ToolStatus::Failed("Security: non-Discord URL".into()),
                };
            }

            const MAX_SIZE: usize = 10 * 1024 * 1024;
            match reqwest::get(&url).await {
                Ok(resp) => {
                    match resp.bytes().await {
                        Ok(bytes) => {
                            let size = bytes.len();
                            if size > MAX_SIZE {
                                return ToolResult {
                                    task_id,
                                    output: format!("Rejected: file is {} bytes, exceeds 10MB safety limit.", size),
                                    tokens_used: 0,
                                    status: ToolStatus::Failed("File too large".into()),
                                };
                            }
                            if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                                let output_text = if text.len() > 30_000 {
                                    format!("{}...\n\n[TRUNCATED: showing first 30KB of {} total bytes]\n[READING INCOMPLETE — The file was truncated at 30KB. If you need more, you cannot use this tool.]", &text[..30_000], size)
                                } else {
                                    format!("{}\n\n[DOCUMENT COMPLETE]", text)
                                };
                                ToolResult {
                                    task_id,
                                    output: format!("--- ATTACHMENT ({} bytes) ---\n{}", size, output_text),
                                    tokens_used: 0,
                                    status: ToolStatus::Success,
                                }
                            } else {
                                ToolResult {
                                    task_id,
                                    output: format!("Binary file ({} bytes). Cannot display as text. The file was inspected in-memory but contains non-UTF8 binary data.", size),
                                    tokens_used: 0,
                                    status: ToolStatus::Success,
                                }
                            }
                        }
                        Err(e) => ToolResult { task_id, output: format!("Failed to read response bytes: {}", e), tokens_used: 0, status: ToolStatus::Failed(e.to_string()) },
                    }
                }
                Err(e) => ToolResult { task_id, output: format!("Failed to fetch attachment: {}. The CDN URL may have expired.", e), tokens_used: 0, status: ToolStatus::Failed(e.to_string()) },
            }
        });
        return Some(handle);
    } 
    
    if tool_type == "manage_user_preferences" {
        let mem_clone = memory.clone();
        let scope_clone = scope.clone();
        let handle = tokio::spawn(async move {
            crate::agent::preferences::execute_manage_user_preferences(task_id, desc, scope_clone, mem_clone, tx_clone).await
        });
        return Some(handle);
    } 
    
    if tool_type == "manage_skill" {
        let mem_clone = memory.clone();
        let is_admin = true;
        let handle = tokio::spawn(async move {
            crate::agent::skills::execute_manage_skill(task_id, desc, is_admin, mem_clone, tx_clone).await
        });
        return Some(handle);
    } 
    
    if tool_type == "manage_routine" {
        let mem_clone = memory.clone();
        let handle = tokio::spawn(async move {
            crate::agent::routines::execute_manage_routine(task_id, desc, mem_clone, tx_clone).await
        });
        return Some(handle);
    } 
    
    if tool_type == "manage_lessons" {
        let mem_clone = memory.clone();
        let scope_clone = scope.clone();
        let handle = tokio::spawn(async move {
            crate::agent::lessons_drone::execute_manage_lessons(task_id, desc, mem_clone, tx_clone, &scope_clone).await
        });
        return Some(handle);
    } 
    
    if tool_type == "search_timeline" {
        let mem_clone = memory.clone();
        let scope_clone = scope.clone();
        let handle = tokio::spawn(async move {
            crate::agent::timeline_drone::execute_search_timeline(task_id, desc, mem_clone, tx_clone, &scope_clone).await
        });
        return Some(handle);
    }

    if tool_type == "manage_scratchpad" {
        let mem_clone = memory.clone();
        let scope_clone = scope.clone();
        let handle = tokio::spawn(async move {
            crate::agent::scratchpad_drone::execute_manage_scratchpad(task_id, desc, mem_clone, tx_clone, &scope_clone).await
        });
        return Some(handle);
    }

    if tool_type == "operate_synaptic_graph" {
        let mem_clone = memory.clone();
        let handle = tokio::spawn(async move {
            crate::agent::synaptic_drone::execute_operate_synaptic_graph(task_id, desc, mem_clone, tx_clone).await
        });
        return Some(handle);
    }

    if tool_type == "read_core_memory" {
        let mem_clone = memory.clone();
        let scope_clone = scope.clone();
        let handle = tokio::spawn(async move {
            crate::agent::core_memory_drone::execute_read_core_memory(task_id, desc, mem_clone, tx_clone, &scope_clone).await
        });
        return Some(handle);
    } 
    
    if tool_type == "synthesizer" {
        let ctx_clone = context.to_string();
        let handle = tokio::spawn(async move {
            crate::agent::synthesis_drone::execute_synthesizer(task_id, desc, ctx_clone, provider, tx_clone).await
        });
        return Some(handle);
    }

    None
}
