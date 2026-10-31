# KeepUp

Single-binary, ultra-lightweight self-hosted uptime monitoring engine and status page.

[![Rust 2021](https://img.shields.io/badge/Rust-2021-0a84ff.svg)](https://www.rust-lang.org)
[![Memory: <15MB Idle](https://img.shields.io/badge/Memory-%3C15MB_Idle-30d158.svg)](#security--footprint)
[![Boot: <20ms](https://img.shields.io/badge/Boot-%3C20ms-30d158.svg)](#architecture)
[![Storage: SQLite WAL](https://img.shields.io/badge/Storage-SQLite_WAL-ffd60a.svg)](#architecture)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

KeepUp compiles synthetic health checking, persistent time-series logging, stateful alert dispatching, and glassmorphic public status pages into a standalone executable with zero external database dependencies.

---

## Quickstart

### Docker (Single Command)

```bash
docker run -d \
  --name keepup \
  -p 3000:3000 \
  -e ADMIN_USER=admin \
  -e ADMIN_PASSWORD=change_me_immediately \
  -v keepup-data:/data \
  ridloghfry/keepup:latest
```

Access endpoints:
- **Admin Dashboard:** `http://localhost:3000/`
- **Public Status Page:** `http://localhost:3000/status`
- **Status API:** `http://localhost:3000/api/public/status`
- **Vector Badge:** `http://localhost:3000/api/public/badge/<monitor-id>.svg`

### Docker Compose (`docker-compose.yml`)

```yaml
version: '3.8'

services:
  keepup:
    image: ridloghfry/keepup:latest
    container_name: keepup
    restart: unless-stopped
    ports:
      - "3000:3000"
    volumes:
      - keepup-data:/data
    environment:
      - PORT=3000
      - ADMIN_USER=admin
      - ADMIN_PASSWORD=change_me_immediately
      - KEEPUP_DB_PATH=/data/keepup.db
      - STATUS_PAGE_THEME=auto
    healthcheck:
      test: ["CMD-SHELL", "curl -f http://localhost:3000/api/public/status || exit 1"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 5s

volumes:
  keepup-data:
```

Launch with:

```bash
docker compose up -d
```

### Cargo Local Build

Prerequisites: Rust 1.82+ and SQLite build libraries.

```bash
git clone https://github.com/RidloGhifary/keepup.git
cd keepup
cargo build --release
./target/release/keepup
```

---

## Core Feature Matrix

### Synthetic Monitoring
- High-frequency HTTP and HTTPS synthetic probes with configurable verification intervals (10s to 3600s).
- Configurable response status code matchers and validation rules.
- Per-target request timeout limits (1s to 60s) preventing thread exhaustion.
- Failure retry thresholds before triggering state transitions, eliminating transient false positives.
- Live response time tracking recorded to local SQLite time-series check logs.

### Alerting & Webhooks
- State-change alerting dispatched only when a monitor transitions between `UP` and `DOWN` states.
- Native integration formats for Discord Rich Embeds, Slack Block Kit messages, and Generic HTTP webhooks.
- Automatic downtime calculation and resolution notifications (`[RESOLVED]`) displaying exact duration metrics.
- Support for custom Handlebars-style mustache templates with dynamic payload interpolation.

### Public Status Page
- Standalone presentation view at `/status` (navigation headers and management controls omitted).
- Interactive 7-day uptime timeline history breakdown per public component.
- Chronological 24-hour hourly latency histogram with proportional metric bars.
- Live embeddable vector status badges (`/api/public/badge/:monitor_id.svg`) with Status Colors (`#30D158` Operational, `#FF453A` Downtime, `#FFD60A` Degraded, `#71717A` Not Found).
- Built-in theme switcher: System Dark (`#0b0b0c`), OLED Pitch Black (`#000000`), Midnight Navy (`#050B14`), and Clean Light (`#f5f5f7`), with automatic OS `prefers-color-scheme` synchronization.

### Security & Footprint
- Standalone single-binary distribution with zero external database dependencies; static web assets embedded via `rust-embed`.
- Sub-15MB idle RAM consumption and cold-boot times under 20ms.
- Single-admin session authentication using signed `HttpOnly`, `SameSite=Lax` 7-day JWT cookies.
- Constant-time password verification via `subtle::ConstantTimeEq` mitigating timing side-channel attacks.
- IP-based brute-force authentication protection enforced via `tower-governor` with reverse proxy IP header resolution (`X-Forwarded-For`, `X-Real-IP`).

---

## Configuration Reference

KeepUp is configured via environment variables or a local `.env` file:

| Variable | Description | Default | Required |
| --- | --- | --- | --- |
| `PORT` | HTTP server binding port | `3000` | No |
| `ADMIN_USER` | Admin login username | `admin` | Yes |
| `ADMIN_PASSWORD` | Admin login password | None | Yes |
| `KEEPUP_DB_PATH` | Path to SQLite database file | `./data/keepup.db` | No |
| `STATUS_PAGE_THEME` | Default status page theme (`auto`, `dark`, `light`, `oled`, `midnight`) | `auto` | No |
| `CUSTOM_LOGO_URL` | Optional custom logo URL for status page | None | No |

---

## Architecture

KeepUp executes as a single compiled binary without external runtime or database processes:

```text
+-------------------------------------------------------------------------+
|                              KeepUp Process                             |
|                                                                         |
|  +------------------------------+     +------------------------------+  |
|  |     Private Dashboard UI     |     |      Public Status Page      |  |
|  |    (Embedded HTML/CSS/JS)    |     |    (Embedded HTML/CSS/JS)    |  |
|  +--------------+---------------+     +--------------+---------------+  |
|                 |                                    |                  |
|                 +-----------------+------------------+                  |
|                                   |                                     |
|                       [ Axum Router & REST API ]                        |
|                                   |                                     |
|                    [ Auth & Rate Limit Middleware ]                     |
|                     (JWT Cookies / tower-governor)                      |
|                                   |                                     |
|                                   v                                     |
|                         [ Tokio Async Engine ]                          |
|                          /                  \                           |
|                         /                    \                          |
|                        v                      v                         |
|             [ Synthetic Scheduler ]      [ Alert Dispatcher ]           |
|              (HTTP/TCP Probe Loop)       (Discord / Slack / Webhook)    |
|                        |                                                |
|                        v                                                |
|             [ SQLite (WAL Mode) ]                                       |
|             (Embedded persistence)                                      |
+-------------------------------------------------------------------------+
```

Data pipeline execution:
1. **Scheduler:** The background Tokio loop fires asynchronous HTTP probes on defined intervals (`Tokio Async Engine`).
2. **API & Delivery:** Inbound API traffic and status requests are routed via `Axum HTTP API`.
3. **Persistence:** Latency, status code, and availability results are committed directly to `SQLite (WAL Mode)` with `PRAGMA journal_mode=WAL;` and `PRAGMA synchronous=NORMAL;`.
4. **State Evaluation:** When state transitions occur (`UP -> DOWN` or `DOWN -> UP`), the alerting dispatcher triggers webhooks with incident duration telemetry.
5. **Presentation:** Axum serves embedded static assets, JSON API endpoints, and live vector badges directly from the single binary.

---

## License

This project is licensed under the [MIT License](LICENSE).
