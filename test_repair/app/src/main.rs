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

fn repair_unescaped_newlines(json: &str) -> String {
    let mut result = String::with_capacity(json.len());
    let mut in_string = false;
    let mut escape_next = false;
    for c in json.chars() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }
        match c {
            '\\' => {
                result.push(c);
                escape_next = true;
            }
            '"' => {
                in_string = !in_string;
                result.push(c);
            }
            '\n' | '\r' if in_string => {
                result.push_str(if c == '\n' { "\\n" } else { "\\r" });
            }
            _ => result.push(c),
        }
    }
    result
}

fn main() {
    let raw = r#"{
    "thought": "I should reply",
    "tasks": [
        {
            "task_id": "1",
            "tool_type": "reply_to_request",
            "description": "Multi
line
string",
            "depends_on": []
        }
    ]
}"#;
    let repaired = repair_unescaped_newlines(raw);
    println!("REPAIRED:\n{}", repaired);
    match serde_json::from_str::<AgentPlan>(&repaired) {
        Ok(_) => println!("PARSE: SUCCESS"),
        Err(e) => println!("PARSE: FAILED ({})", e),
    }
}
