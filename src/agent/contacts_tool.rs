use crate::models::tool::{ToolResult, ToolStatus};
use tokio::sync::mpsc;
use crate::agent::preferences::extract_tag;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discord_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

const CONTACTS_PATH: &str = "memory/contacts.json";

async fn load_contacts() -> Vec<Contact> {
    match tokio::fs::read_to_string(CONTACTS_PATH).await {
        Ok(json_str) => serde_json::from_str(&json_str).unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    }
}

async fn save_contacts(contacts: &[Contact]) -> Result<(), String> {
    let _ = std::fs::create_dir_all("memory");
    let json = serde_json::to_string_pretty(contacts).map_err(|e| e.to_string())?;
    tokio::fs::write(CONTACTS_PATH, json).await.map_err(|e| e.to_string())
}

pub async fn execute_contacts(
    task_id: String,
    description: String,
    telemetry_tx: Option<mpsc::Sender<String>>,
) -> ToolResult {
    let action = extract_tag(&description, "action:").unwrap_or_else(|| "list".to_string());

    macro_rules! telemetry {
        ($tx:expr, $msg:expr) => {
            if let Some(ref tx) = $tx {
                let _ = tx.send($msg).await;
            }
        };
    }

    match action.as_str() {
        "add" => {
            let name = match extract_tag(&description, "name:") {
                Some(n) if !n.is_empty() => n,
                _ => return ToolResult { task_id, output: "Error: 'name:' is required.".into(), tokens_used: 0, status: ToolStatus::Failed("Missing name".into()) },
            };

            telemetry!(telemetry_tx, format!("  → Adding contact: {}...\n", name));

            let now = chrono::Utc::now().to_rfc3339();
            let contact = Contact {
                id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
                name: name.clone(),
                email: extract_tag(&description, "email:"),
                discord_id: extract_tag(&description, "discord_id:"),
                phone: extract_tag(&description, "phone:"),
                notes: extract_tag(&description, "notes:"),
                tags: extract_tag(&description, "tags:")
                    .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_default(),
                created_at: now.clone(),
                updated_at: now,
            };

            let mut contacts = load_contacts().await;
            contacts.push(contact.clone());
            if let Err(e) = save_contacts(&contacts).await {
                return ToolResult { task_id, output: format!("Error saving: {}", e), tokens_used: 0, status: ToolStatus::Failed("FS Error".into()) };
            }

            telemetry!(telemetry_tx, "  ✅ Contact saved.\n".into());
            ToolResult {
                task_id,
                output: format!("Contact '{}' added (id: {}). Total contacts: {}", name, contact.id, contacts.len()),
                tokens_used: 0,
                status: ToolStatus::Success,
            }
        }

        "list" => {
            telemetry!(telemetry_tx, "  → Loading contact list...\n".into());
            let contacts = load_contacts().await;
            if contacts.is_empty() {
                return ToolResult { task_id, output: "No contacts found.".into(), tokens_used: 0, status: ToolStatus::Success };
            }

            let mut output = format!("📒 {} contacts:\n", contacts.len());
            for c in &contacts {
                output.push_str(&format!("\n• {} (id: {})", c.name, c.id));
                if let Some(ref email) = c.email { output.push_str(&format!(" | ✉ {}", email)); }
                if let Some(ref discord) = c.discord_id { output.push_str(&format!(" | 🎮 {}", discord)); }
                if let Some(ref phone) = c.phone { output.push_str(&format!(" | 📱 {}", phone)); }
                if !c.tags.is_empty() { output.push_str(&format!(" | 🏷 {}", c.tags.join(", "))); }
                if let Some(ref notes) = c.notes { output.push_str(&format!("\n  Notes: {}", notes)); }
            }

            ToolResult { task_id, output, tokens_used: 0, status: ToolStatus::Success }
        }

        "search" => {
            let query = match extract_tag(&description, "query:") {
                Some(q) if !q.is_empty() => q.to_lowercase(),
                _ => return ToolResult { task_id, output: "Error: 'query:' is required.".into(), tokens_used: 0, status: ToolStatus::Failed("Missing query".into()) },
            };

            telemetry!(telemetry_tx, format!("  → Searching contacts for '{}'...\n", query));
            let contacts = load_contacts().await;
            let matches: Vec<&Contact> = contacts.iter().filter(|c| {
                c.name.to_lowercase().contains(&query)
                    || c.email.as_ref().map_or(false, |e| e.to_lowercase().contains(&query))
                    || c.discord_id.as_ref().map_or(false, |d| d.to_lowercase().contains(&query))
                    || c.phone.as_ref().map_or(false, |p| p.contains(&query))
                    || c.tags.iter().any(|t| t.to_lowercase().contains(&query))
                    || c.notes.as_ref().map_or(false, |n| n.to_lowercase().contains(&query))
            }).collect();

            if matches.is_empty() {
                return ToolResult { task_id, output: format!("No contacts matching '{}'.", query), tokens_used: 0, status: ToolStatus::Success };
            }

            let mut output = format!("Found {} match(es) for '{}':\n", matches.len(), query);
            for c in &matches {
                output.push_str(&format!("\n• {} (id: {})", c.name, c.id));
                if let Some(ref email) = c.email { output.push_str(&format!(" | ✉ {}", email)); }
                if let Some(ref discord) = c.discord_id { output.push_str(&format!(" | 🎮 {}", discord)); }
                if let Some(ref phone) = c.phone { output.push_str(&format!(" | 📱 {}", phone)); }
            }

            ToolResult { task_id, output, tokens_used: 0, status: ToolStatus::Success }
        }

        "update" => {
            let id = match extract_tag(&description, "id:") {
                Some(i) if !i.is_empty() => i,
                _ => return ToolResult { task_id, output: "Error: 'id:' is required.".into(), tokens_used: 0, status: ToolStatus::Failed("Missing id".into()) },
            };

            telemetry!(telemetry_tx, format!("  → Updating contact {}...\n", id));
            let mut contacts = load_contacts().await;
            let contact = match contacts.iter_mut().find(|c| c.id == id) {
                Some(c) => c,
                None => return ToolResult { task_id, output: format!("No contact with id '{}'.", id), tokens_used: 0, status: ToolStatus::Failed("Not found".into()) },
            };

            if let Some(name) = extract_tag(&description, "name:") { contact.name = name; }
            if let Some(email) = extract_tag(&description, "email:") { contact.email = Some(email); }
            if let Some(discord) = extract_tag(&description, "discord_id:") { contact.discord_id = Some(discord); }
            if let Some(phone) = extract_tag(&description, "phone:") { contact.phone = Some(phone); }
            if let Some(notes) = extract_tag(&description, "notes:") { contact.notes = Some(notes); }
            if let Some(tags) = extract_tag(&description, "tags:") {
                contact.tags = tags.split(',').map(|s| s.trim().to_string()).collect();
            }
            contact.updated_at = chrono::Utc::now().to_rfc3339();

            let name = contact.name.clone();
            if let Err(e) = save_contacts(&contacts).await {
                return ToolResult { task_id, output: format!("Error saving: {}", e), tokens_used: 0, status: ToolStatus::Failed("FS Error".into()) };
            }

            telemetry!(telemetry_tx, "  ✅ Contact updated.\n".into());
            ToolResult { task_id, output: format!("Contact '{}' (id: {}) updated.", name, id), tokens_used: 0, status: ToolStatus::Success }
        }

        "delete" => {
            let id = match extract_tag(&description, "id:") {
                Some(i) if !i.is_empty() => i,
                _ => return ToolResult { task_id, output: "Error: 'id:' is required.".into(), tokens_used: 0, status: ToolStatus::Failed("Missing id".into()) },
            };

            telemetry!(telemetry_tx, format!("  → Deleting contact {}...\n", id));
            let mut contacts = load_contacts().await;
            let before = contacts.len();
            contacts.retain(|c| c.id != id);

            if contacts.len() == before {
                return ToolResult { task_id, output: format!("No contact with id '{}'.", id), tokens_used: 0, status: ToolStatus::Failed("Not found".into()) };
            }

            if let Err(e) = save_contacts(&contacts).await {
                return ToolResult { task_id, output: format!("Error saving: {}", e), tokens_used: 0, status: ToolStatus::Failed("FS Error".into()) };
            }

            telemetry!(telemetry_tx, "  ✅ Contact deleted.\n".into());
            ToolResult { task_id, output: format!("Contact '{}' deleted. {} contacts remaining.", id, contacts.len()), tokens_used: 0, status: ToolStatus::Success }
        }

        _ => ToolResult {
            task_id,
            output: format!("Error: Unknown action '{}'. Use: add, list, search, update, delete.", action),
            tokens_used: 0,
            status: ToolStatus::Failed("Bad Action".into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_add_contact() {
        let r = execute_contacts("1".into(), "action:[add] name:[John Doe] email:[john@example.com] tags:[friend, dev]".into(), None).await;
        assert_eq!(r.status, ToolStatus::Success);
        assert!(r.output.contains("John Doe"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_add_missing_name() {
        let r = execute_contacts("1".into(), "action:[add]".into(), None).await;
        assert!(matches!(r.status, ToolStatus::Failed(_)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_list_contacts() {
        let r = execute_contacts("1".into(), "action:[list]".into(), None).await;
        assert_eq!(r.status, ToolStatus::Success);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_search_contacts() {
        let r = execute_contacts("1".into(), "action:[search] query:[nonexistent]".into(), None).await;
        assert_eq!(r.status, ToolStatus::Success);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_delete_nonexistent() {
        let r = execute_contacts("1".into(), "action:[delete] id:[fake123]".into(), None).await;
        assert!(matches!(r.status, ToolStatus::Failed(_)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_bad_action() {
        let r = execute_contacts("1".into(), "action:[explode]".into(), None).await;
        assert!(matches!(r.status, ToolStatus::Failed(_)));
    }
}
