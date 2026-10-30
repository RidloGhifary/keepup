<div align="center">

# ⚡ KeepUp

**Ultra-lightweight, single-binary uptime monitoring & public status page engine powered by Rust.**

[![License: MIT](https://img.shields.io/badge/License-MIT-30d158.svg)](LICENSE)
[![Rust: 1.82+](https://img.shields.io/badge/Rust-1.82%2B-0a84ff.svg)](https://www.rust-lang.org)
[![RAM: <15MB](https://img.shields.io/badge/Memory-%3C15MB_RAM-30d158.svg)](#performance--footprint)
[![Docker Image: ~25MB](https://img.shields.io/badge/Docker_Image-~25MB-0a84ff.svg)](#docker--deployment)
[![Design: Apple HIG](https://img.shields.io/badge/Aesthetics-Apple_Dark_Mode-ffd60a.svg)](#apple-human-interface-aesthetics)

<p align="center">
  <a href="#key-features">Key Features</a> •
  <a href="#quickstart-with-docker">Quickstart</a> •
  <a href="#docker-compose">Docker Compose</a> •
  <a href="#local-development">Development</a> •
  <a href="#configuration">Configuration</a> •
  <a href="#architecture">Architecture</a>
</p>

</div>

---

## ✦ Overview

**KeepUp** is designed for indie hackers, DevOps engineers, and teams who want reliable, high-frequency uptime monitoring and an elegant public status page without the overhead of heavy enterprise tools. 

Written from the ground up in asynchronous Rust, KeepUp runs with **under 15MB of RAM**, requires **zero external database setup**, and compiles its entire user interface directly into a single standalone binary.

```text
  ██╗  ██╗███████╗███████╗██████╗ ██╗   ██╗██████╗ 
  ██║ ██╔╝██╔════╝██╔════╝██╔══██╗██║   ██║██╔══██╗
  █████╔╝ █████╗  █████╗  ██████╔╝██║   ██║██████╔╝
  ██╔═██╗ ██╔══╝  ██╔══╝  ██╔═══╝ ██║   ██║██╔═══╝ 
  ██║  ██╗███████╗███████╗██║     ╚██████╔╝██║     
  ╚═╝  ╚═╝╚══════╝╚══════╝╚═╝      ╚═════╝ ╚═╝     
  Ultra-lightweight Uptime & Telemetry Engine
```

---

## 🚀 Key Features

* **⚡ Extreme Resource Efficiency**  
  Built with Tokio and Axum. Operates with sub-15MB RAM idle consumption and sub-1% CPU usage across dozens of concurrent targets.
* **📦 Single Binary & Zero Dependencies**  
  No separate Node.js runtime, Redis server, or external database required. All HTML, CSS, JavaScript, and database migrations are baked directly into the executable via `rust-embed` and `sqlx`.
* **🎨 Apple Human Interface Aesthetics**  
  Translucent glassmorphism (`backdrop-blur-2xl`), deep dark mode (`#0b0b0c`), custom San Francisco typography, glowing pulse status indicators, and responsive SVG latency charts.
* **💾 Embedded SQLite in WAL Mode**  
  Self-contained time-series telemetry store running `PRAGMA journal_mode=WAL;` and `PRAGMA synchronous=NORMAL;` for high-throughput concurrent logging without database locking.
* **🔔 Stateful, Spam-Free Alerting**  
  Immediate alerts to **Discord**, **Slack**, and custom **Generic Webhooks** only when a service transitions between `UP` and `DOWN` states (with configurable retry thresholds to eliminate false positives).
* **🌐 Public Status Page**  
  Built-in customer-facing status page at `/status` showcasing aggregated system operational health, grouped components, and 90-day daily uptime history bars.
* **🔒 Single-Admin Security & Rate Limiting**  
  Secure authentication via environment-configured credentials, signed 7-day `HttpOnly` JWT cookies with constant-time password verification (`subtle`), and IP-based brute-force rate limiting (`tower-governor`).
* **🔔 Apple-Style Toast Notifications**  
  Spring-animated toast alerts and centralized HTTP error handling for `401 Unauthorized`, `429 Rate Limits`, and server interruptions.

---

## ⚡ Quickstart with Docker

Run KeepUp with persistent storage in a single command:

```bash
docker run -d \
  --name keepup \
  -p 3000:3000 \
  -v ./data:/app/data \
  -e ADMIN_USER=admin \
  -e ADMIN_PASSWORD=supersecret \
  ghcr.io/yourusername/keepup:latest
```

Open your browser:
* **Admin Dashboard:** [http://localhost:3000/](http://localhost:3000/)
* **Public Status Page:** [http://localhost:3000/status](http://localhost:3000/status)

---

## 🐳 Docker Compose

For automated production deployments, use the included [`docker-compose.yml`](docker-compose.yml):

```yaml
version: '3.8'

services:
  keepup:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: keepup
    restart: unless-stopped
    ports:
      - "3000:3000"
    volumes:
      - ./data:/app/data
    environment:
      - PORT=3000
      - RUST_LOG=info,keepup=debug
      - KEEPUP_DB_PATH=/app/data/keepup.db
      - DATABASE_URL=sqlite:///app/data/keepup.db?mode=rwc
      - ADMIN_USER=admin
      - ADMIN_PASSWORD=supersecret
      - JWT_SECRET=change-this-in-production-keepup-jwt-secret
    healthcheck:
      test: ["CMD-SHELL", "curl -f http://localhost:3000/api/public/status || exit 1"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 5s
```

Start the service:

```bash
docker compose up -d
```

---

## 🛠️ Local Development

### Prerequisites
* [Rust](https://rustup.rs/) (1.82 or newer recommended)
* SQLite3 build dependencies (`pkg-config`, `libssl-dev`)

### Step-by-Step Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/yourusername/keepup.git
   cd keepup
   ```

2. **Configure Environment:**
   Copy or edit the default `.env` file:
   ```bash
   cp .env.example .env
   ```

3. **Run the application:**
   ```bash
   cargo run
   ```

4. **Build an optimized release binary:**
   ```bash
   cargo build --release
   ./target/release/keepup
   ```

5. **Run test suite:**
   ```bash
   cargo test
   ```

---

## ⚙️ Environment Variables Reference

| Variable | Description | Default | Example |
| :--- | :--- | :--- | :--- |
| `PORT` | TCP port the web server binds to | `3000` | `8080` |
| `KEEPUP_DB_PATH` | Path to the SQLite database file | `data/keepup.db` | `/app/data/keepup.db` |
| `DATABASE_URL` | Standard SQLite connection string | `sqlite:///app/data/keepup.db?mode=rwc` | `sqlite://data/custom.db` |
| `ADMIN_USER` | Administrator username for dashboard login | `admin` | `ops_lead` |
| `ADMIN_PASSWORD` | Administrator password for dashboard login | `keepup123` | `P@ssw0rd987!` |
| `JWT_SECRET` | Secret key used to sign session cookies | Auto-generated 64-char UUID | `k33pup_s3cur1ty_k3y_s3cr3t` |
| `RUST_LOG` | Tracing & logging filter level | `info,keepup=debug` | `debug,sqlx=warn` |

---

## 📐 System Architecture

KeepUp consolidates web serving, background scheduling, embedded persistence, and notification dispatching into an asynchronous event loop inside a single process:

```text
+---------------------------------------------------------------------------------+
|                                  KeepUp Binary                                  |
|                                                                                 |
|  +-------------------------------------+  +----------------------------------+  |
|  |       Private Admin UI (Auth)       |  |      Public Status Page (Public)  |  |
|  |  (SPA / Embedded HTML, CSS, JS)     |  |   (SPA / Embedded HTML, CSS, JS) |  |
|  +------------------+------------------+  +------------------+---------------+  |
|                     |                                        |                  |
|                     +-------------------+--------------------+                  |
|                                         |                                       |
|                              [ Axum Router & REST API ]                         |
|                                         |                                       |
|                       [ Auth & Rate Limiting Middleware ]                       |
|                          (JWT Cookies & tower-governor)                         |
|                                         |                                       |
|                             [ In-Memory AppState ]                              |
|                          (Arc<AppState> + Channels)                             |
|                                         |                                       |
|                    +--------------------+--------------------+                  |
|                    |                                         |                  |
|        [ Tokio Scheduler Pool ]                    [ Alerting Dispatcher ]      |
|        (Async Reqwest / Ping)                                |                  |
|                    |                                         v                  |
|        [ Async SQLite Flush Writer ]              Discord & Slack Webhooks      |
|        (PRAGMA journal_mode=WAL)                                                |
|                    |                                                            |
|                    v                                                            |
|           data/keepup.db                                                        |
+---------------------------------------------------------------------------------+
```

---

## 🛡️ Security & Hardening

* **Constant-Time Verification:** Login requests evaluate credentials using `subtle::ConstantTimeEq` to prevent timing attacks.
* **HttpOnly Session Cookies:** Authentication tokens are stored inside strict `HttpOnly`, `SameSite=Lax`, `Path=/` cookies inaccessible to malicious client scripts.
* **Brute-Force Protection:** The `POST /api/auth/login` route is protected by `tower-governor`, enforcing a 5-request burst threshold and replenishment rate of 1 request per 12 seconds per client IP.
* **Proxy-Aware Extraction:** Compatible with reverse proxies via `SmartIpKeyExtractor` inspecting `X-Forwarded-For` and `X-Real-IP` headers.

---

## 🤝 Contributing

Contributions, feature requests, and bug reports are welcome!
1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

---

## 📄 License

This project is open-source software licensed under the **[MIT License](LICENSE)**.
