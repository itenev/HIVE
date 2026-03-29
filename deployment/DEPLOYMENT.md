# HIVE — Container Deployment Guide

## Overview

This setup runs HIVE and Ollama together inside a single Docker container.
The container has internet access (required for Discord and web search) but is
isolated from your local network via a firewall rule.

---

## Prerequisites

- Docker 24+ with Compose v2 (`docker compose`, not `docker-compose`). Docker 27+
  includes buildx as a built-in — no separate plugin install needed.
- Enough RAM for your chosen model (qwen3.5:9b ≈ 6 GB; qwen3:32b Q4 ≈ 24 GB)
- Linux host strongly recommended; macOS works but the firewall step differs

### GPU support (optional but highly recommended)

CPU-only inference processes the 84KB system prompt in 60-90 seconds. GPU drops this
to 3-10 seconds. If you have an NVIDIA GPU:

**Step 1 — Install NVIDIA Container Toolkit:**

```bash
sudo apt-get install -y nvidia-container-toolkit
sudo nvidia-ctk runtime configure --runtime=docker
sudo systemctl restart docker
```

**Step 2 — Build and run with the GPU overlay:**

```bash
cd deployment
docker compose -f docker-compose.yml -f docker-compose.gpu.yml build
docker compose -f docker-compose.yml -f docker-compose.gpu.yml up -d
```

> The Rust binary is cached from the builder stage — only the runtime stage rebuilds
> when you switch between CPU/GPU overlays.

**Step 3 — Verify GPU is visible inside the container:**

```bash
docker compose exec hive nvidia-smi
```

Without GPU, run in CPU-only mode (use smaller models like `qwen3.5:4b` or `qwen3.5:1.5b`):

```bash
docker compose build
docker compose up -d
```

---

## Directory layout

All deployment files live in the `deployment/` subdirectory of the cloned HIVE repo:

```
HIVE/
├── deployment/
│   ├── Dockerfile             ← multi-stage build (Rust + runtime)
│   ├── docker-compose.yml     ← base config (CPU-only)
│   ├── docker-compose.gpu.yml ← GPU overlay (CUDA + NVIDIA passthrough)
│   ├── entrypoint.sh          ← startup script (Ollama → warmup → fix perms → HIVE)
│   ├── .env                   ← your secrets (gitignored)
│   ├── setup_firewall.sh      ← optional LAN isolation helper
│   └── DEPLOYMENT.md          ← this file
├── src/
├── Cargo.toml
└── ...
```

---

## Step 1 — Create your `.env` file

From the repo root, copy the example and fill in your values:

```bash
cp deployment/.env deployment/.env
nano deployment/.env
```

**All documented env vars are listed in `.env` with their defaults.** Only uncomment
and set values that differ from the defaults for your deployment.

**Required (no defaults):**

```env
DISCORD_TOKEN=your_d...here                    # Discord bot token
HIVE_ADMIN_USERS=your_discord_user_id_here     # Comma-separated admin user IDs
HIVE_CHAT_CHANNEL=your_listen_channel_id       # Channel Apis listens to
HIVE_TARGET_CHANNEL=your_post_channel_id       # Channel Apis posts events to
```

> **Do NOT commit `.env` to git.** The `.gitignore` excludes it automatically.

---

## Step 2 — Choose your model

Set `HIVE_MODEL` in your `.env`. The entrypoint pulls it automatically on first start.
Upstream default: `qwen3.5:35b`.

| Model           | RAM / VRAM needed | Notes                                    |
|-----------------|-------------------|-----------------------------------------|
| `qwen3.5:4b`    | ~4 GB             | Recommended for 6 GB GPU (RTX 2060)      |
| `qwen3.5:9b`    | ~6 GB             | Needs 8+ GB VRAM or fast CPU            |
| `qwen3.5:35b`   | ~22 GB            | Upstream default. Needs 24 GB+ VRAM     |
| `qwen3:14b`     | ~12 GB            | Better reasoning, needs 16 GB+          |
| `qwen3:8b`      | ~6 GB             | Fast, works on 8 GB machines             |
| `qwen3:32b`     | ~24 GB            | Best quality, needs 24 GB+              |
| `llama3.2:3b`   | ~2 GB             | Lightweight testing only                 |

> **6 GB GPU note:** `qwen3.5:4b` is the largest model that fits in 6 GB VRAM
> (RTX 2060, 1060). Larger models fall back to CPU automatically but will be slow.

The model is cached in the `ollama-models` Docker volume and persists across
container rebuilds and `docker compose down`.

---

## Step 3 — Block LAN access at the firewall

Docker's bridge network gives the container internet access but also lets it reach
your local subnet. Add this iptables rule to block that while keeping internet working:

### Linux (iptables)

```bash
# Find your LAN subnet (typically 192.168.x.0/24 or 10.x.x.0/24)
ip route | grep -v default

# Replace 192.168.1.0/24 with YOUR actual LAN subnet
sudo iptables -I DOCKER-USER -s 172.28.0.0/16 -d 192.168.1.0/24 -j DROP

# Make it persist across reboots (Debian/Ubuntu)
sudo apt install iptables-persistent
sudo netfilter-persistent save
```

### macOS (pf firewall)

```bash
# Add to /etc/pf.conf — replace en0 and 192.168.1.0/24 with your values
echo "block from 172.28.0.0/16 to 192.168.1.0/24" | sudo tee -a /etc/pf.conf
sudo pfctl -f /etc/pf.conf -e
```

Verify it works:

```bash
# This should FAIL (LAN blocked)
docker run --rm --network deployment_hive-net alpine ping -c2 192.168.1.1

# This should SUCCEED (internet reachable)
docker run --rm --network deployment_hive-net alpine wget -qO- https://icanhazip.com
```

---

## Step 4 — Build and start

```bash
cd deployment

# First build: compiles Rust (3-5 min) + installs Ollama + pulls model
docker compose build
docker compose up -d

# Watch logs
docker compose logs -f
```

Subsequent starts (after code changes):

```bash
cd deployment
docker compose build
docker compose up -d
```

Updating from upstream:

```bash
cd HIVE
git pull
cd deployment
docker compose build
docker compose up -d
```

> If you see an unexpectedly small binary (415 KB instead of ~49 MB), run
> `docker builder prune -f` before building to clear stale cache.

CLI-only mode (no Discord needed):

```bash
docker compose run --rm hive
```

---

## Step 5 — Monitor and maintain

```bash
# Check resource usage
docker stats hive

# View HIVE logs
docker compose logs hive

# Exec into container (you'll be the hive user, not root)
docker compose exec hive bash

# Inspect the binary inside the container
docker compose exec hive bash -c "ls -la /app/hive"

# Stop
docker compose down

# Full reset (WARNING: deletes memory, logs, and model cache)
docker compose down -v
```

### Getting the public tunnel URL

The container auto-starts a Cloudflare quick tunnel on port 8420.
Grab the URL from inside the container:

```bash
docker compose exec hive cat /app/memory/core/tunnel_url.txt
```

---

## Environment variables reference

> **All vars are listed in `docker-compose.yml` with upstream defaults.**
> Copy `.env` and override only the values you need to change.
> Vars marked with `[NEW]` are added to the compose template but not in the upstream `.env.example`.

### Discord

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DISCORD_TOKEN` | Yes | — | Discord bot token |
| `HIVE_ADMIN_USERS` | Yes | — | Comma-separated admin Discord user IDs |
| `HIVE_CHAT_CHANNEL` | Yes | — | Channel Apis listens to for public messages |
| `HIVE_TARGET_CHANNEL` | Yes | — | Channel Apis posts autonomy events to |

### Model & Provider

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `HIVE_MODEL` | No | `qwen3.5:35b` | Main inference model |
| `HIVE_GLASSES_MODEL` | No | `qwen3.5:35b` | Glasses platform model |
| `HIVE_PROVIDER` [NEW] | No | `ollama` | Provider: `ollama`, `openai`, `anthropic`, `gemini`, `xai` |
| `HIVE_OLLAMA_URL` | No | `http://localhost:11434` | Ollama server URL |
| `OLLAMA_HOST` [NEW] | No | `http://localhost:11434` | Alias for `HIVE_OLLAMA_URL` (some subsystems) |

### Ollama Runtime (consumed by Ollama server inside container)

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `OLLAMA_NUM_PARALLEL` | No | `1` | Ollama parallelism. Set to `1` for models without parallel support |
| `OLLAMA_MAX_QUEUE` | No | `32` | Ollama request queue depth |
| `OLLAMA_KV_CACHE_TYPE` | No | (none/f16) | KV cache quantization: `q4_0`, `q8_0`, `f16`, etc. |

### HIVE Core

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `HIVE_MAX_PARALLEL` | No | `2` | Max concurrent ReAct loops |
| `HIVE_PYTHON_BIN` | No | `python3` | Python binary for image generation / training |
| `RUST_LOG` | No | `info` | Log level: `error`, `warn`, `info`, `debug`, `trace` |

### Sleep Training

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `HIVE_SLEEP_BATCH` [NEW] | No | `2` | Micro-batch size for identity reflection |
| `HIVE_SLEEP_INTERVAL` [NEW] | No | `43200` | Auto-sleep interval in seconds (43200 = 12 h) |
| `HIVE_SLEEP_LR` [NEW] | No | `1e-5` | Micro-training learning rate |

### Identity & Networking

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `HIVE_USER_NAME` [NEW] | No | `anonymous` | Display name for P2P human mesh |
| `HIVE_HUMAN_MESH` [NEW] | No | (disabled) | Enable P2P mesh — set to `true` to enable |
| `HIVE_HUMAN_MESH_PORT` [NEW] | No | `9877` | P2P mesh listen port |
| `OUTREACH_CHANNEL_ID` [NEW] | No | — | Enable outbound message routing to this channel |

### Paths & Ports

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `HIVE_CACHE_DIR` [NEW] | No | `memory/cache/images` | Image cache directory |
| `HIVE_FILE_SERVER_PORT` | No | `8420` | File server HTTP port |
| `HIVE_FILE_TOKEN` | No | (dev mode) | File server auth token |

### Smart Home & Email

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SMART_HOME_URL` [NEW] | No | — | Philips Hue / OpenHue bridge URL |
| `SMART_HOME_TOKEN` [NEW] | No | — | Smart home auth token |
| `IMAP_HOST/PORT/USER/PASS` [NEW] | No | — | IMAP inbound email |
| `SMTP_HOST/PORT/USER/PASS` [NEW] | No | — | SMTP outbound email |

### Provider API Keys

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `BRAVE_SEARCH_API_KEY` | No | — | Enables web search tool |
| `ANTHROPIC_API_KEY` | If using | — | Required if `HIVE_PROVIDER=anthropic` |
| `OPENAI_API_KEY` | If using | — | Required if `HIVE_PROVIDER=openai` |
| `GEMINI_API_KEY` | If using | — | Required if `HIVE_PROVIDER=gemini` |
| `XAI_API_KEY` | If using | — | Required if `HIVE_PROVIDER=xai` |

---

## Security properties of this setup

| Property                          | Status | Notes                                         |
|-----------------------------------|--------|-----------------------------------------------|
| No host filesystem access         | ✅     | Only named volumes, no bind mounts            |
| No host network access           | ✅     | Custom bridge network                         |
| LAN isolated                      | ✅*    | After Step 3 firewall rule                    |
| Runs as non-root                 | ✅     | `hive` user inside container                  |
| No privilege escalation          | ✅     | `no-new-privileges:true`                      |
| All Linux caps dropped           | ✅     | NET_BIND_SERVICE, CHOWN, DAC_OVERRIDE, SETUID, SETGID retained |
| Resource limits enforced         | ✅     | Memory + CPU caps in compose file             |
| Internet access (Discord, search)| ✅     | Required for core functionality               |
| Cloudflare quick tunnel          | ✅     | Auto-starts on port 8420                     |
| Shell tool (`run_bash_command`)   | ⚠️     | Still works — contained to the container      |
| File tool (`file_system_operator`)| ⚠️    | Writes only to /app/data volume              |

*LAN isolation requires the manual iptables/pf step above.

The shell and file tools still function inside the container — that's intentional,
as they're core to HIVE's design. The containment means any damage is limited to
the container's volumes, not your host machine.

---

## Troubleshooting

**Container exits immediately:** Check `docker compose logs hive`. Common causes:

- Missing `DISCORD_TOKEN` in `.env` — the bot token is required even in CLI mode
- Missing `HIVE_ADMIN_USERS` — at least one admin user ID must be set
- Missing `HIVE_TARGET_CHANNEL` — the bot needs a target channel ID

**Binary is 415 KB instead of ~49 MB (stale cache):** Docker may be reusing a cached
stub build. Fix:

```bash
docker builder prune -f
docker compose build
```

**Out of memory:** Reduce the model size or increase the `memory` limit in
docker-compose.yml. qwen3:32b needs at least 28 GB to avoid swapping.

**Ollama not starting:** The Ollama install script inside Docker occasionally
fails on non-standard base images. If so, verify you're using `debian:bookworm-slim`
exactly as specified in the Dockerfile.

**Model pull times out:** The 180s start_period in the healthcheck may not be
enough for large models on slow connections. Increase it or pull the model
manually first:

```bash
docker compose run --rm hive ollama pull qwen3:32b
```
