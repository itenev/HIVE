/// Repair common LLM JSON malformations from the Planner output.
/// Strips markdown fences, BOM, trailing commas, and extracts JSON from conversational preamble.
#[cfg(not(tarpaulin_include))]
pub fn repair_planner_json(raw: &str) -> String {
    let mut s = raw.trim().to_string();

    // Strip BOM
    s = s.trim_start_matches('\u{feff}').to_string();

    // Check if there is a json code block within conversational text
    let json_start_marker = "```json";
    let generic_start_marker = "```";

    if let Some(start_idx) = s.find(json_start_marker) {
        // Found a ```json block, extract everything after the marker
        s = s[start_idx + json_start_marker.len()..].to_string();
        // Find the first closing fence (not rfind)
        if let Some(end_idx) = s.find("```") {
            s = s[..end_idx].to_string();
        }
    } else if let Some(start_idx) = s.find(generic_start_marker) {
         // Found a generic ``` block
        s = s[start_idx + generic_start_marker.len()..].to_string();
        if let Some(end_idx) = s.find("```") {
             s = s[..end_idx].to_string();
        }
    }

    s = s.trim().to_string();

    let mut candidates = Vec::new();
    let mut brace_level = 0;
    let mut start_idx = None;

    for (i, c) in s.char_indices() {
        if c == '{' {
            if brace_level == 0 {
                start_idx = Some(i);
            }
            brace_level += 1;
        } else if c == '}' {
            brace_level -= 1;
            if brace_level == 0 {
                if let Some(start) = start_idx {
                    let candidate = &s[start..=i];
                    
                    // Fix trailing commas before closing braces/brackets: ,} or ,]
                    let mut cleaned = candidate.to_string();
                    while cleaned.contains(",}") { cleaned = cleaned.replace(",}", "}"); }
                    while cleaned.contains(",]") { cleaned = cleaned.replace(",]", "]"); }
                    while cleaned.contains(", }") { cleaned = cleaned.replace(", }", "}"); }
                    while cleaned.contains(", ]") { cleaned = cleaned.replace(", ]", "]"); }

                    if serde_json::from_str::<crate::agent::planner::AgentPlan>(&cleaned).is_ok() {
                        return cleaned;
                    } else if serde_json::from_str::<serde_json::Value>(&cleaned).is_ok() {
                        candidates.push(cleaned);
                    }
                }
            }
            if brace_level < 0 { brace_level = 0; }
        }
    }

    if let Some(first_valid) = candidates.first() {
        return first_valid.clone();
    }

    // Fallback: If no valid JSON was extracted, return empty so the caller can trigger the formatting error prompt.
    String::new()
}
