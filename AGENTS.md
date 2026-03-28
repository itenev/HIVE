# HIVE Agent Instructions

This document provides operational guidance for AI agents working on the HIVE project.

---

## Project Overview

**HIVE** is a sovereign, fully-local AI agent runtime written in pure Rust. It powers **Apis** — an autonomous AI persona that thinks, acts, remembers, and evolves.

- **Language**: Rust (100%)
- **Source Files**: ~130 Rust modules
- **Lines of Code**: ~22K (22K Rust, 187 Python)
- **Test Coverage**: 200+ tests (all passing)
- **Framework**: Pure trait-based architecture (zero external frameworks)
- **LLM Provider**: Ollama (local) with pluggable providers (OpenAI, Anthropic, Gemini, xAI)

---

## Repository Structure

```
HIVE/
├── src/
│   ├── agent/          # ReAct loop, tool registry, 43 tool implementations
│   ├── computer/       # Turing grid, PDF generation, file server
│   ├── engine/         # Core event loop, drives, goals, inbox, telemetry
│   ├── memory/         # 5-tier memory store (working, scratchpad, timeline, synaptic, lessons)
│   ├── models/         # Message, Scope, Capabilities, Tool definitions
│   ├── network/        # NeuroLease P2P mesh, Human P2P mesh
│   ├── platforms/      # Platform abstractions (Discord, CLI, Glasses)
│   ├── prompts/        # System prompt builder (kernel, identity, HUD)
│   ├── providers/      # LLM provider interface (Ollama, OpenAI, Anthropic, Gemini, xAI)
│   ├── server/         # Memory visualizer (Axum HTTP server)
│   ├── teacher/        # Self-supervised learning (preference pairs, golden examples)
│   ├── voice/          # Kokoro TTS integration
│   ├── main.rs         # Entry point, initialization sequence
│   └── lib.rs          # JNI bridge for Android
├── Cargo.toml          # Dependencies (tokio, serenity, reqwest, etc.)
├── memory/             # Runtime memory storage
├── logs/               # Rotating logs (daily, max 8 files)
├── start_hive.sh       # Quick start script
└── .env                # Configuration (Discord token, API keys)
```

---

## Key Architecture Concepts

### The 5-Tier Memory Store
1. **Working Memory** — Current session context
2. **Scratchpad** — Ephemeral per-user scratch space
3. **Timeline** — Conversation archives
4. **Synaptic Graph** — Associative knowledge links
5. **Lessons** — Permanent learned knowledge

### Scope Security Model
- `Scope::Public` — Public channels, CLI
- `Scope::Private { user_id }` — DMs, per-user isolated data
- Memory queries pass through `Scope::can_read()` — enforced before reaching LLM context
- **Security is at the memory layer, not prompt layer**

### The ReAct Loop
- Located in `src/engine/react.rs` (737 lines)
- Multi-turn reasoning: Think → Act → Observe → Repeat
- Tool selection via LLM with streaming token extraction
- `<think>` tag parsing for reasoning traces

### Platform Abstraction
- `trait Platform` — Any messaging interface implements `start()` and `send()`
- Current implementations: Discord, CLI, Glasses (Meta Ray-Ban)
- Adding new platforms = one `impl Platform`

### Admin Users (hardcoded in `main.rs`)
- `"1299810741984956449"` — primary admin
- `"1282286389953695745"` — secondary admin
- `"1473412348105457786"` — admin
- `"local_admin"` — CLI access
- `"apis_autonomy"` — Autonomy loop with full tool access

---

## Development Workflow

### Building
```bash
cargo build --release    # Full release build
cargo build              # Debug build (faster)
cargo check             # Fast type check only
```

### Testing
```bash
cargo test --all         # Run all tests
cargo test --lib         # Library tests only
```

### Running
```bash
./start_hive.sh          # With Discord (requires .env)
cargo run --release      # CLI-only mode
RUST_LOG=debug cargo run # Verbose logging
```

### Environment Variables
| Variable | Required | Description |
|----------|----------|-------------|
| `DISCORD_TOKEN` | For Discord | Discord bot token |
| `BRAVE_SEARCH_API_KEY` | No | Web search |
| `HIVE_PROVIDER` | No | `ollama`, `openai`, `anthropic`, `gemini`, `xai` (default: `ollama`) |
| `HIVE_PYTHON_BIN` | No | Python path for image generation |
| `HIVE_FILE_SERVER_PORT` | No | File server port (default: 8420) |
| `OLLAMA_BASE_URL` | No | Ollama endpoint (default: `http://localhost:11434`) |
| `RUST_LOG` | No | Log level (default: `warn,HIVE=info`) |

### Default Ollama Model
- `qwen3:32b` — default model
- Glasses platform uses `qwen3.5:35b`

---

## Tool Development Guidelines

### Adding a New Tool
1. Create `src/agent/my_tool.rs` implementing `Tool` trait
2. Register in `src/agent/mod.rs`: `pub mod my_tool;`
3. Add to `AgentManager::new()` tool registry
4. Add test in `src/agent/tests.rs`

### Tool Trait (`src/agent/tool.rs`)
```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn execute(&self, context: &str, args: Value) -> ToolResult;
}
```

### Available Tool Categories
- **Information**: `web_search`, `researcher`, `codebase_list`, `codebase_read`, `read_attachment`, `channel_reader`, `read_logs`
- **Memory**: `manage_user_preferences`, `store_lesson`, `manage_scratchpad`, `operate_synaptic_graph`, `review_reasoning`
- **Execution**: `operate_turing_grid` (Python, JS, Rust, Swift, Ruby, Perl, AppleScript), `run_bash_command`, `process_manager`, `file_system_operator`
- **Creation**: `image_generator`, `kokoro_tts`, `file_writer` (PDF/multi-format), `synthesizer`
- **Automation**: `manage_routine`, `manage_skill`, `manage_goals`, `tool_forge`
- **Communication**: `emoji_react`, `outreach`, `email_tool`, `calendar_tool`, `contacts_tool`
- **Advanced**: `autonomy_tool`, `moderation_tool`, `compiler_tool`, `spawner`, `opencode`, `sub_agent`

---

## Adding a New LLM Provider

1. Create `src/providers/my_provider.rs`
2. Implement `trait Provider`:
```rust
#[async_trait]
impl Provider for MyProvider {
    async fn generate(
        &self,
        system_prompt: &str,
        history: &[Event],
        event: &Event,
        tools: &[ToolTemplate],
    ) -> Result<String, ProviderError>;
}
```
3. Add provider selection in `main.rs` `run_app()`:
```rust
"myprovider" => Arc::new(MyProvider::new()...)
```

---

## Adding a New Platform

1. Create `src/platforms/my_platform.rs`
2. Implement `trait Platform`:
```rust
#[async_trait]
impl Platform for MyPlatform {
    fn name(&self) -> &str { "myplatform" }
    async fn start(&self, event_sender: Sender<Event>) -> Result<(), PlatformError>;
    async fn send(&self, response: Response) -> Result<(), PlatformError>;
}
```
3. Register in `EngineBuilder::new().with_platform(Box::new(MyPlatform::new()))`

---

## Daemons (spawned at startup)
- **File Server** (port 8420) — Serves generated/downloaded files over HTTP
- **Cloudflare Tunnel** — Public URL for file access
- **Memory Visualizer** (port 3030) — Brain state visualization
- **IMAP Inbox Watcher** — Background email polling
- **Chronos** — Temporal operations daemon
- **Uptime Checkpoint** — Saves state every 5 minutes

---

## Logging

- **Location**: `logs/hive.log.YYYY-MM-DD`
- **Rotation**: Daily, max 8 files
- **Format**: `[SUBSYSTEM] message`
- **Subsystems**: `[ENGINE:*]`, `[MEMORY:*]`, `[AGENT:*]`, `[PROVIDER:*]`, `[TUNNEL:*]`, `[FILE SERVER:*]`

---

## Troubleshooting

### Visualizer Server Fails
- Check port 3030: `lsof -i :3030`
- `spawn_visualizer_server()` silently fails during `tokio::spawn`

### File Permissions
- Ensure `.env` has 600: `chmod 600 .env`

### Turing Grid Artifacts
- Python code may fail with SyntaxError after write
- Always read cell content before executing previously written cells

---

## Configuration Files

| File | Purpose |
|------|---------|
| `.env` | Secrets (Discord token, API keys) |
| `.env.example` | Template for `.env` |
| `memory/` | Runtime memory storage |
| `logs/` | Application logs |
| `gauntlet_admin.txt` | Admin verification flag |

---

## Git Workflow

- **Remote**: `git@github.com:itenev/HIVE.git`
- **Default branch**: main
- Recent commits include: memory fixes, OpenCode integration, NeuroLease mesh, observer loop fixes

---

## Important Files for Deep Work

| File | Lines | Purpose |
|------|-------|---------|
| `src/engine/react.rs` | 737 | ReAct loop core |
| `src/engine/mod.rs` | ~500 | Engine builder, initialization |
| `src/agent/mod.rs` | ~400 | AgentManager, tool registry |
| `src/memory/mod.rs` | ~300 | 5-tier memory store |
| `src/prompts/kernel.rs` | ~200 | Zero Assumption Protocol |
| `src/platforms/discord/mod.rs` | ~400 | Discord handler, telemetry |

---

## Quick Reference

### Common Commands
```bash
# Build
cargo build --release

# Test
cargo test --all

# Run (CLI only)
cargo run --release

# Check code
cargo clippy

# Format
cargo fmt
```

### Rust Edition
- Edition: 2024 (latest)

### Key Dependencies
- `tokio` — Async runtime
- `serenity` — Discord API
- `reqwest` — HTTP client
- `serde` — Serialization
- `tracing` — Structured logging
- `neo4rs` — Neo4j graph database
