#![allow(clippy::collapsible_if)]
use reqwest::Client;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::models::message::Event;
use super::{Provider, ProviderError};

#[derive(Serialize, Deserialize, Clone)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

/// A provider implementation for a local Ollama instance.
pub struct OllamaProvider {
    client: Client,
    endpoint: String,
    model: String,
}

impl OllamaProvider {
    /// Connects to a local Ollama instance defaulting to `qwen3.5:35b` as requested.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            endpoint: "http://localhost:11434/api/chat".to_string(),
            model: "qwen3.5:35b".to_string(),
        }
    }
    fn map_chunk_err(e: reqwest::Error) -> ProviderError {
        ProviderError::ConnectionError(e.to_string())
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    #[tracing::instrument(skip(self, system_prompt, history, telemetry_tx), fields(model=%self.model, user=%new_event.author_name))]
    async fn generate(
        &self,
        system_prompt: &str,
        history: &[Event],
        new_event: &Event,
        agent_context: &str,
        telemetry_tx: Option<mpsc::Sender<String>>,
    ) -> Result<String, ProviderError> {
        let mut messages = Vec::new();

        // Format the securely-scoped history FIRST
        // Individual messages are capped to prevent one massive response from bloating all subsequent calls.
        // Full messages remain in working memory & disk — only the LLM prompt copy is capped.
        const HISTORY_MSG_CAP: usize = 2000;
        for event in history {
            let role = if event.author_name == "Apis" {
                "assistant"
            } else {
                "user"
            };

            // Inject the author name softly into user messages so Apis knows who is talking
            let content = if role == "user" {
                format!("{}: {}", event.author_name, event.content)
            } else {
                // SPRINT 3: JSON Content Forcing
                // If Apis responded in plain text, wrap it into a mock `reply_to_request` execution.
                // This prevents "Monkey See, Monkey Do" plain text degradation on Turn 1.
                if !event.content.trim().starts_with("```json") && !event.content.trim().starts_with('{') {
                    // Properly escape the original string into a JSON string
                    let escaped_content = serde_json::to_string(&event.content).unwrap_or_else(|_| "\"Failed to escape\"".to_string());
                    format!(
                        "```json\n{{\n  \"tasks\": [\n    {{\n      \"task_id\": \"hist_1\",\n      \"tool_type\": \"reply_to_request\",\n      \"description\": {},\n      \"depends_on\": []\n    }}\n  ]\n}}\n```",
                        escaped_content
                    )
                } else {
                    event.content.clone()
                }
            };

            // Cap oversized history messages to prevent prompt bloat from prior mega-responses.
            let capped_content = if content.len() > HISTORY_MSG_CAP {
                let truncated: String = content.chars().take(HISTORY_MSG_CAP).collect();
                format!("{}...\n[Message truncated from {} to {} chars for context efficiency. Full version retained in memory.]", truncated, content.len(), HISTORY_MSG_CAP)
            } else {
                content
            };

            messages.push(OllamaMessage {
                role: role.to_string(),
                content: capped_content,
                images: None,
            });
        }

        // Pinned System Prompt: Load the strict operational rules AFTER the history
        // to combat LLM "lost in the middle" recency bias on massive context windows.
        messages.push(OllamaMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
            images: None,
        });

        let mut final_user_message = format!("{}: {}", new_event.author_name, new_event.content);
        if !agent_context.is_empty() {
            final_user_message.push_str("\n\n[ISOLATED EXECUTION TIMELINE]\n");
            final_user_message.push_str(agent_context);
        }

        // Native Vision Support: Extract image URLs from [USER_ATTACHMENT] tags and fetch them
        let mut b64_images = Vec::new();
        let attachment_blocks: Vec<&str> = final_user_message.split("[USER_ATTACHMENT:").skip(1).collect();
        for block in attachment_blocks {
            if let Some(end_idx) = block.find(']') {
                let tag_content = &block[..end_idx];
                let is_image = tag_content.contains("type: image/");
                if is_image {
                    if let Some(url_start) = tag_content.find("url: ") {
                        let url = tag_content[url_start + 5..].trim();
                        if let Ok(resp) = self.client.get(url).send().await {
                            if let Ok(bytes) = resp.bytes().await {
                                use base64::{Engine as _, engine::general_purpose::STANDARD};
                                b64_images.push(STANDARD.encode(&bytes));
                            }
                        }
                    }
                }
            }
        }

        let images_opt = if b64_images.is_empty() { None } else { Some(b64_images) };

        // Strict enforcement for Turn 1 "Monkey see, monkey do" conversational degradation
        final_user_message.push_str("\n\n[SYSTEM ENFORCEMENT: You must output EXACTLY ONE valid JSON block. Do not output raw conversational text. Use the `reply_to_request` tool to speak to the user.]");

        // Add the current triggering event
        messages.push(OllamaMessage {
            role: "user".to_string(),
            content: final_user_message,
            images: images_opt,
        });

        let payload = OllamaRequest {
            model: self.model.clone(),
            messages,
            stream: true,
        };

        let mut res = self.client.post(&self.endpoint)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionError(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(ProviderError::ParseError(format!("Ollama error {}: {}", status, text)));
        }

        let mut first_token_received = false;
        let mut full_response = String::new();
        let mut raw_buffer = String::new();
        let mut final_prompt_tokens = 0;
        let mut final_eval_tokens = 0;
        let mut ttft_duration = tokio::time::Duration::from_secs(0);
        let prompt_bytes: usize = payload.messages.iter().map(|m| m.content.len()).sum();
        let start_time = tokio::time::Instant::now();

        while let Some(chunk) = res.chunk().await.map_err(Self::map_chunk_err)? {
            let chunk_str = String::from_utf8_lossy(&chunk);
            raw_buffer.push_str(&chunk_str);

            while let Some(newline_pos) = raw_buffer.find('\n') {
                let line: String = raw_buffer.drain(..=newline_pos).collect();
                let line_trimmed = line.trim();
                if line_trimmed.is_empty() {
                    continue;
                }

                // If JSON line parses, extract tokens
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line_trimmed) {
                    if let Some(msg) = parsed.get("message") {
                        if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                            full_response.push_str(content);
                        }

                        // Some models stream reasoning separately in a 'thinking' key
                        if let Some(thinking) = msg.get("thinking").and_then(|v| v.as_str()) {
                            if let Some(ref tx) = telemetry_tx {
                                if !thinking.is_empty() {
                                    let _ = tx.send(thinking.to_string()).await;
                                }
                            }
                        }
                    }

                    if parsed.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
                        final_prompt_tokens = parsed.get("prompt_eval_count").and_then(|v| v.as_u64()).unwrap_or(0);
                        final_eval_tokens = parsed.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0);
                        break;
                    }

                    if !first_token_received && parsed.get("message").is_some() {
                        ttft_duration = start_time.elapsed();
                        first_token_received = true;
                    }
                } else {
                    return Err(ProviderError::ParseError("Failed to parse JSON stream chunk".into()));
                }
            }
        }

        let total_time = start_time.elapsed();
        let metrics = crate::engine::telemetry::LatencyMetrics {
            timestamp: chrono::Utc::now().to_rfc3339(),
            model: self.model.clone(),
            prompt_bytes,
            history_len: history.len(),
            ttft_ms: ttft_duration.as_millis() as u64,
            total_ms: total_time.as_millis() as u64,
            prompt_tokens: final_prompt_tokens,
            eval_tokens: final_eval_tokens,
        };

        tokio::spawn(async move {
            crate::engine::telemetry::log_latency(metrics).await;
        });

        Ok(full_response.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::scope::Scope;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_provider_success() {
        let mock_server = MockServer::start().await;
        
        let mut provider = OllamaProvider::new();
        provider.endpoint = format!("{}/api/chat", mock_server.uri());

        let mock_response = "{\"message\": {\"role\": \"assistant\", \"content\": \"Sure, here's your context.\"}, \"done\": true}\n";

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(mock_response))
            .mount(&mock_server)
            .await;

        let history = vec![
            Event { platform: "cli".into(), scope: Scope::Public { channel_id: "t".into(), user_id: "t".into() }, author_name: "Apis".into(), author_id: "test".into(), content: "I am here.".into() },
            Event { platform: "cli".into(), scope: Scope::Public { channel_id: "t".into(), user_id: "t".into() }, author_name: "Alice".into(), author_id: "test".into(), content: "Hi!".into() },
        ];
        
        // Single JSON response is technically a 1-line stream chunk
        let new_event = Event {
            platform: "cli".into(),
            scope: Scope::Public { channel_id: "t".into(), user_id: "t".into() },
            author_name: "Bob".into(),
            author_id: "test".into(),
            content: "What's up?".into(),
        };
        let res = provider.generate("sys", &history, &new_event, "", None).await.unwrap();

        assert_eq!(res, "Sure, here's your context.");
    }

    #[tokio::test]
    async fn test_provider_http_error() {
        let mock_server = MockServer::start().await;
        
        let mut provider = OllamaProvider::new();
        provider.endpoint = format!("{}/api/chat", mock_server.uri());

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let res = provider.generate("sys", &[], &Event {
            platform: "cli".into(),
            scope: Scope::Public { channel_id: "t".into(), user_id: "t".into() },
            author_name: "Bob".into(),
            author_id: "test".into(),
            content: "Bork?".into(),
        }, "", None).await;

        assert!(matches!(res, Err(ProviderError::ParseError(_))));
    }

    #[tokio::test]
    async fn test_provider_connection_error() {
        let mut provider = OllamaProvider::new();
        provider.endpoint = "http://invalid.domain.that.does.not.exist:1234/api/chat".into();

        let res = provider.generate("sys", &[], &Event {
            platform: "cli".into(),
            scope: Scope::Public { channel_id: "t".into(), user_id: "t".into() },
            author_name: "Bob".into(),
            author_id: "test".into(),
            content: "Bork?".into(),
        }, "", None).await;

        assert!(matches!(res, Err(ProviderError::ConnectionError(_))));
    }

    #[tokio::test]
    async fn test_provider_parse_error() {
        let mock_server = MockServer::start().await;
        let mut provider = OllamaProvider::new();
        provider.endpoint = format!("{}/api/chat", mock_server.uri());

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string("invalid json body!\n"))
            .mount(&mock_server)
            .await;

        let res = provider.generate("sys", &[], &Event {
            platform: "cli".into(),
            scope: Scope::Public { channel_id: "t".into(), user_id: "t".into() },
            author_name: "Bob".into(),
            author_id: "test".into(),
            content: "Bork?".into(),
        }, "", None).await;

        assert!(matches!(res, Err(ProviderError::ParseError(_))));
    }

    #[tokio::test]
    async fn test_provider_early_eof() {
        let mock_server = MockServer::start().await;
        let mut provider = OllamaProvider::new();
        provider.endpoint = format!("{}/api/chat", mock_server.uri());

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&mock_server)
            .await;

        // No chunks, natural EOF. 
        let res = provider.generate("sys", &[], &Event {
            platform: "cli".into(),
            scope: Scope::Public { channel_id: "t".into(), user_id: "t".into() },
            author_name: "Bob".into(),
            author_id: "test".into(),
            content: "Bork?".into(),
        }, "", None).await;

        assert_eq!(res.unwrap(), "");
    }

    #[tokio::test]
    async fn test_provider_reasoning_telemetry() {
        let mock_server = MockServer::start().await;
        let mut provider = OllamaProvider::new();
        provider.endpoint = format!("{}/api/chat", mock_server.uri());

        let mock_response = "{\"message\": {\"role\": \"assistant\", \"thinking\": \"I am thinking...\", \"content\": \"Final answer\"}, \"done\": true}\n";

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(mock_response))
            .mount(&mock_server)
            .await;

        let (tx, mut rx) = mpsc::channel(10);
        
        let res = provider.generate("sys", &[], &Event {
            platform: "cli".into(),
            scope: Scope::Public { channel_id: "t".into(), user_id: "t".into() },
            author_name: "Bob".into(),
            author_id: "test".into(),
            content: "Bork?".into(),
        }, "", Some(tx)).await;

        let first_recv = rx.recv().await.unwrap();
        assert_eq!(first_recv, "I am thinking...");
        assert_eq!(res.unwrap(), "Final answer");
    }

    #[tokio::test]
    async fn test_provider_missing_content() {
        let mock_server = MockServer::start().await;
        let mut provider = OllamaProvider::new();
        provider.endpoint = format!("{}/api/chat", mock_server.uri());

        let mock_response = "{\"message\": {\"role\": \"assistant\"}, \"done\": true}\n";

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(mock_response))
            .mount(&mock_server)
            .await;

        let res = provider.generate("sys", &[], &Event {
            platform: "cli".into(),
            scope: Scope::Public { channel_id: "t".into(), user_id: "t".into() },
            author_name: "Bob".into(),
            author_id: "test".into(),
            content: "Bork?".into(),
        }, "", None).await;

        assert_eq!(res.unwrap(), "");
    }

    #[tokio::test]
    async fn test_ollama_stream_fragmented() {
        let mock_server = MockServer::start().await;
        let mut provider = OllamaProvider::new();
        provider.endpoint = format!("{}/api/chat", mock_server.uri());

        let mock_response = "{\"message\": {\"role\": \"assistant\", \"content\": \"part1\"}}\n{\"message\": {\"content\": \" part2\"}}\n{\"message\": {\"content\": \" done!\"}, \"done\": true}\n";

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(mock_response))
            .mount(&mock_server)
            .await;

        let res = provider.generate("sys", &[], &Event {
            platform: "cli".into(),
            scope: Scope::Public { channel_id: "t".into(), user_id: "t".into() },
            author_name: "Bob".into(),
            author_id: "test".into(),
            content: "Stream?".into(),
        }, "", None).await;

        assert_eq!(res.unwrap(), "part1 part2 done!");
    }

    #[tokio::test]
    async fn test_ollama_stream_disconnect() {
        let mock_server = MockServer::start().await;
        let mut provider = OllamaProvider::new();
        provider.endpoint = format!("{}/api/chat", mock_server.uri());

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(503).set_body_string("Service Unavailable Drops Stream"))
            .mount(&mock_server)
            .await;

        let res = provider.generate("sys", &[], &Event {
            platform: "cli".into(),
            scope: Scope::Public { channel_id: "t".into(), user_id: "t".into() },
            author_name: "Bob".into(),
            author_id: "test".into(),
            content: "Disconnect?".into(),
        }, "", None).await;

        assert!(res.is_err());
    }
}
