use crate::models::tool::{ToolResult, ToolStatus};
use crate::agent::preferences::extract_tag;
use tokio::sync::mpsc;

/// Tool that provides project creator and contributor information.
/// Uses git history to determine development timeline and contributors,
/// keeping personal identity data out of prompts and kernels.
pub async fn execute_contributors(
    task_id: String,
    description: String,
    telemetry_tx: Option<mpsc::Sender<String>>,
) -> ToolResult {
    let action = extract_tag(&description, "action:").unwrap_or_else(|| "info".to_string());

    macro_rules! telemetry {
        ($tx:expr, $msg:expr) => {
            if let Some(ref tx) = $tx {
                let _ = tx.send($msg).await;
            }
        };
    }

    match action.as_str() {
        "info" | "creator" | "about" => {
            telemetry!(telemetry_tx, "  📋 Fetching project creator and contributor info...\n".into());

            // Static creator info
            let mut output = String::new();
            output.push_str("## HIVE Project — Creator & Contributors\n\n");
            output.push_str("### Creator\n");
            output.push_str("- **Name:** Maria Smith\n");
            output.push_str("- **GitHub:** MettaMazza\n");
            output.push_str("- **Discord:** metta_mazza\n");
            output.push_str("- **Role:** Lead Developer / Architect\n\n");

            // Git-derived development timeline
            // Use --format=%ai to get author dates (survive rebase/force-push)
            // Sort all dates to find TRUE first and latest, not just HEAD order
            output.push_str("### Development Timeline (from git history)\n");

            // Get ALL commits with author date, hash, subject — sorted by author date
            match tokio::process::Command::new("git")
                .args(["log", "--all", "--format=%ai|%H|%s", "--date-order"])
                .output()
                .await
            {
                Ok(res) => {
                    let raw = String::from_utf8_lossy(&res.stdout).to_string();
                    let mut lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
                    if !lines.is_empty() {
                        // Sort by author date (first field) to find true earliest/latest
                        lines.sort();
                        let first = lines.first().unwrap();
                        let latest = lines.last().unwrap();
                        output.push_str(&format!("- **First commit (by author date):** {}\n", first.replace('|', " ")));
                        output.push_str(&format!("- **Latest commit (by author date):** {}\n", latest.replace('|', " ")));

                        // Calculate span
                        if let (Some(first_date), Some(latest_date)) = (first.split('|').next(), latest.split('|').next()) {
                            output.push_str(&format!("- **Development span:** {} → {}\n", first_date.trim(), latest_date.trim()));
                        }
                    }
                }
                Err(_) => output.push_str("- Commit history: unavailable\n"),
            }

            // Total commits
            match tokio::process::Command::new("git")
                .args(["rev-list", "--count", "HEAD"])
                .output()
                .await
            {
                Ok(res) => {
                    let count = String::from_utf8_lossy(&res.stdout).trim().to_string();
                    output.push_str(&format!("- **Total commits:** {}\n", count));
                }
                Err(_) => {}
            }

            output.push_str("\n### All Contributors (from git shortlog)\n");

            // All contributors with commit counts
            match tokio::process::Command::new("git")
                .args(["shortlog", "-sne", "HEAD"])
                .output()
                .await
            {
                Ok(res) => {
                    let contributors = String::from_utf8_lossy(&res.stdout).to_string();
                    if contributors.trim().is_empty() {
                        output.push_str("- No contributors found in git history.\n");
                    } else {
                        for line in contributors.lines() {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                output.push_str(&format!("- {}\n", trimmed));
                            }
                        }
                    }
                }
                Err(_) => output.push_str("- Git shortlog unavailable.\n"),
            }

            ToolResult {
                task_id,
                output,
                tokens_used: 0,
                status: ToolStatus::Success,
            }
        }
        _ => ToolResult {
            task_id,
            output: format!("Unknown action '{}'. Use action:[info] to get creator and contributor details.", action),
            tokens_used: 0,
            status: ToolStatus::Failed("Bad Action".into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_contributors_info() {
        let r = execute_contributors("1".into(), "action:[info]".into(), None).await;
        assert!(matches!(r.status, ToolStatus::Success));
        assert!(r.output.contains("Maria Smith"));
        assert!(r.output.contains("MettaMazza"));
        assert!(r.output.contains("Creator"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_contributors_bad_action() {
        let r = execute_contributors("1".into(), "action:[explode]".into(), None).await;
        assert!(matches!(r.status, ToolStatus::Failed(_)));
    }
}
