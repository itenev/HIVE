use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmPlan {
    pub tasks: Vec<SwarmTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmTask {
    pub task_id: String,
    pub drone_type: String,
    pub description: String,
    pub depends_on: Vec<String>,
}

pub const PLANNER_SYSTEM_PROMPT: &str = r#"You are the Swarm Queen Planner. You do not accomplish the user's ultimate task yourself. Instead, your objective is to analyze the task and delegate it to specialized Worker Drones.

AVAILABLE DRONES:
{available_drones}

If the user's request is simple (like a greeting, a brief question, or something that requires zero external capability), output an empty task list. We do not spawn drones for simple chat. 

If the request is complex, break it down into parallel or sequential tasks.

OUTPUT FORMAT MUST BE VALID JSON:
{
  "tasks": [
    {
      "task_id": "step_1",
      "drone_type": "researcher",
      "description": "Find specific data about XYZ.",
      "depends_on": [] 
    }
  ]
}
"#;
