use serenity::builder::CreateAttachment;

/// Parses markdown attachment tags like `[ATTACH_IMAGE](path)`, strips them out of the given text,
/// and returns a list of local files formatted as Discord CreateAttachments.
#[cfg(not(tarpaulin_include))]
pub async fn extract_attachments(parsed_text: &mut String) -> Vec<CreateAttachment> {
    let mut attachments = Vec::new();

    // Parse [ATTACH_IMAGE](...) custom markdown tags
    while let Some(start_idx) = parsed_text.find("[ATTACH_IMAGE](") {
        if let Some(end_idx) = parsed_text[start_idx..].find(")") {
            let path_start = start_idx + 15;
            let path_end = start_idx + end_idx;
            let file_path = &parsed_text[path_start..path_end];
            
            match serenity::builder::CreateAttachment::path(file_path).await {
                Ok(attachment) => attachments.push(attachment),
                Err(e) => tracing::error!("[Discord Platform] Failed to attach local image at {}: {}", file_path, e),
            }
            
            // Strip the tag from the output text so the user doesn't see raw markdown
            let before = &parsed_text[..start_idx];
            let after = &parsed_text[start_idx + end_idx + 1..];
            *parsed_text = format!("{}{}", before, after);
        } else {
            break;
        }
    }

    // Parse [ATTACH_FILE](...) custom markdown tags (PDFs, docs, etc.)
    while let Some(start_idx) = parsed_text.find("[ATTACH_FILE](") {
        if let Some(end_idx) = parsed_text[start_idx..].find(")") {
            let path_start = start_idx + 14; // len("[ATTACH_FILE](") = 14
            let path_end = start_idx + end_idx;
            let file_path = &parsed_text[path_start..path_end];
            
            match serenity::builder::CreateAttachment::path(file_path).await {
                Ok(attachment) => attachments.push(attachment),
                Err(e) => tracing::error!("[Discord Platform] Failed to attach local file at {}: {}", file_path, e),
            }
            
            let before = &parsed_text[..start_idx];
            let after = &parsed_text[start_idx + end_idx + 1..];
            *parsed_text = format!("{}{}", before, after);
        } else {
            break;
        }
    }

    // Parse [ATTACH_AUDIO](...) custom markdown tags (TTS wav files)
    while let Some(start_idx) = parsed_text.find("[ATTACH_AUDIO](") {
        if let Some(end_idx) = parsed_text[start_idx..].find(")") {
            let path_start = start_idx + 15; // len("[ATTACH_AUDIO](") = 15
            let path_end = start_idx + end_idx;
            let file_path = &parsed_text[path_start..path_end];
            
            match serenity::builder::CreateAttachment::path(file_path).await {
                Ok(attachment) => attachments.push(attachment),
                Err(e) => tracing::error!("[Discord Platform] Failed to attach audio at {}: {}", file_path, e),
            }
            
            let before = &parsed_text[..start_idx];
            let after = &parsed_text[start_idx + end_idx + 1..];
            *parsed_text = format!("{}{}", before, after);
        } else {
            break;
        }
    }

    attachments
}
