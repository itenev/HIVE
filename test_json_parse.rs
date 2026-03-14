use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlan {
    pub thought: Option<String>,
    pub tasks: Vec<AgentTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub task_id: String,
    pub tool_type: String,
    pub description: String,
    pub depends_on: Vec<String>,
}

fn extract_json_braces(s: &str) -> Option<String> {
    let mut brace_count = 0;
    let mut start_idx = None;

    for (i, c) in s.char_indices() {
        if c == '{' {
            if brace_count == 0 {
                start_idx = Some(i);
            }
            brace_count += 1;
        } else if c == '}' {
            brace_count -= 1;
            if brace_count == 0 {
                if let Some(start) = start_idx {
                    let candidate = &s[start..=i];
                    // Also strip trailing commas
                    let mut cleaned = candidate.to_string();
                    while cleaned.contains(",}") { cleaned = cleaned.replace(",}", "}"); }
                    while cleaned.contains(",]") { cleaned = cleaned.replace(",]", "]"); }
                    while cleaned.contains(", }") { cleaned = cleaned.replace(", }", "}"); }
                    while cleaned.contains(", ]") { cleaned = cleaned.replace(", ]", "]"); }
                    if let Ok(_) = serde_json::from_str::<AgentPlan>(&cleaned) {
                        return Some(cleaned);
                    }
                }
            }
            if brace_count < 0 {
                brace_count = 0;
            }
        }
    }
    None
}

fn repair_planner_json(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    s = s.trim_start_matches('\u{feff}').to_string();

    // Check if there is a json code block within conversational text
    let json_start_marker = "```json";
    let generic_start_marker = "```";

    if let Some(start_idx) = s.find(json_start_marker) {
        s = s[start_idx + json_start_marker.len()..].to_string();
        if let Some(end_idx) = s.find("```") {
            s = s[..end_idx].to_string();
        }
    } else if let Some(start_idx) = s.find(generic_start_marker) {
        s = s[start_idx + generic_start_marker.len()..].to_string();
        if let Some(end_idx) = s.find("```") {
            s = s[..end_idx].to_string();
        }
    }
    
    if let Some(json) = extract_json_braces(&s) {
        return json;
    }
    
    // Fallback if the extracted string was malformed or missing braces
    s
}

fn main() {
    let sample = r#"I should run the web_search tool to check {data}.
{
  "thought": "testing",
  "tasks": [
    {
       "task_id": "1",
       "tool_type": "web_search",
       "description": "test",
       "depends_on": []
    }
  ]
}
And some trailing {text}.
"#;
    println!("Res:\n{}", repair_planner_json(sample));
}
