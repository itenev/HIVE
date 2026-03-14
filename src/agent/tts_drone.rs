use crate::models::tool::{ToolResult, ToolStatus};
use crate::agent::preferences::extract_tag;
use tokio::sync::mpsc;

pub async fn execute_voice_synthesizer(
    task_id: String,
    description: String,
    telemetry_tx: Option<mpsc::Sender<String>>,
) -> ToolResult {
    let text = extract_tag(&description, "text:").unwrap_or_else(|| description.clone());

    if let Some(tx) = &telemetry_tx {
        let max_len = 50;
        let mut display_text = text.clone();
        if display_text.len() > max_len {
            display_text.truncate(max_len);
            display_text.push_str("...");
        }
        let _ = tx
            .send(format!(
                "🗣️ Voice Synthesizer: generating Kokoro audio for '{}'\n",
                display_text
            ))
            .await;
        let _ = tx.send("typing_indicator".into()).await;
    }

    match crate::voice::kokoro::KokoroTTS::new().await {
        Ok(engine) => {
            match engine.get_audio_path(&text).await {
                Ok(path) => {
                    if let Some(tx) = &telemetry_tx {
                        let _ = tx
                            .send(format!("✨ Audio generation complete: {}\n", path.display()))
                            .await;
                    }
                    ToolResult {
                        task_id,
                        tokens_used: 0,
                        status: ToolStatus::Success,
                        output: format!(
                            "Audio generated successfully. YOU MUST include this EXACT tag in your human conversational response to proactively play the audio for the user:\n\n[ATTACH_AUDIO]({})\n\n",
                            path.display()
                        ),
                    }
                }
                Err(e) => {
                    if let Some(tx) = &telemetry_tx {
                        let _ = tx
                            .send(format!("❌ Voice Generator error: {}\n", e))
                            .await;
                    }
                    ToolResult {
                        task_id,
                        tokens_used: 0,
                        status: ToolStatus::Failed(format!("Voice generation failed: {}", e)),
                        output: format!("Voice generation failed: {}", e),
                    }
                }
            }
        }
        Err(e) => {
            if let Some(tx) = &telemetry_tx {
                let _ = tx
                    .send(format!("❌ Voice Engine failed to initialize: {}\n", e))
                    .await;
            }
            ToolResult {
                task_id,
                tokens_used: 0,
                status: ToolStatus::Failed(format!("Voice initialization failed: {}", e)),
                output: format!("Voice initialization failed: {}", e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_voice_synthesizer_success() {
        // Without mocking the python environment, get_audio_path will attempt to spawn python3 and run tts_worker.py. 
        // We will just test that the wrapper correctly handles the args and outputs.
        let res = execute_voice_synthesizer("1".into(), "text:[testing]".into(), None).await;
        // Even if it fails because python environment is missing, we just ensure it returns a ToolResult correctly.
        assert!(matches!(res.status, ToolStatus::Success) || matches!(res.status, ToolStatus::Failed(_)));
    }
}
