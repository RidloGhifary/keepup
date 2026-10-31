# Product Requirements Document (PRD)

## Project Title: KeepUp

**Document Version:** 1.0.0

**Status:** Approved for Implementation

**Author:** Lead Systems Architect & Product Manager

**Target Execution Environment:** Linux / macOS / Windows / Docker (`x86_64`, `aarch64`)

## 1. Document Overview & Goals

### 1.1 Purpose

This document defines the complete product specifications, architecture, functional requirements, database design, and non-functional goals for **KeepUp**—an ultra-lightweight, high-performance, single-binary uptime monitor and public status page engine written in Rust.

### 1.2 Mission Statement

Provide self-hosters, indie hackers, and small-to-medium enterprise teams with a blazingly fast, zero-dependency uptime monitoring solution that operates with sub-15MB RAM idle consumption, instant page loads, built-in public status pages, and native alert dispatches to Discord and Slack.

## 2. Target Audience & Personas

| Persona                    | Role & Context                                                    | Key Needs                                                                                               |
| -------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| **DevOps / HomeLab Admin** | Manages self-hosted services on low-resource VPS or Raspberry Pi. | Minimal memory footprint, single-file SQLite database, Docker deployment, instant startup.              |
| **Indie Developer**        | Operates SaaS products and public APIs.                           | Beautiful public status page, custom domain support, low latency graphs, immediate alert triggers.      |
| **Team Lead / CTO**        | Needs visibility into internal microservice health.               | Reliable alert delivery, failure thresholds to avoid false positives, multi-protocol checks (HTTP/TCP). |

## 3. High-Level System Architecture

KeepUp follows a single-binary architecture where the web server (`Axum`), async task runtime (`Tokio`), static Web UI assets (`rust-embed`), embedded time-series datastore (`SQLite`), and notification engine run in a single process.

```
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
|                             [ In-Memory State Cache ]                           |
|                            (Arc<RwLock<AppState>>)                              |
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

## 4. Key Functional Features & Specifications

### 4.1 Health Check Engine

- **Protocols Supported:**
  - **HTTP / HTTPS:** GET, HEAD, or POST requests with custom header support, body keyword verification, and expected HTTP status code ranges (e.g., `200-299`).

  - **TCP Port Ping:** Validates low-level socket connections on specific ports (e.g., `5432` for Postgres, `22` for SSH).

  - **ICMP Ping:** Basic network-level reachability ping (optional feature branch).

- **Execution Lifecycle:**
  - Configurable check intervals per monitor (5s, 10s, 30s, 60s, 5m, 15m, etc.).

  - Configurable network timeout per target (default: `5` seconds).

  - Configurable retry/failure threshold (e.g., target must fail `3` consecutive times before state changes from `UP` to `DOWN`).

### 4.2 Admin Dashboard (Authenticated)

- **Real-time Overview:** Live grid and list view of all monitored targets showing current status (`UP`, `DEGRADED`, `DOWN`, `PAUSED`), response latency (ms), and 24-hour uptime percentage.

- **Detailed Analytics View:**
  - Latency breakdown graph over configurable time ranges (1 hour, 24 hours, 7 days, 30 days, 90 days).

  - Incident timeline logs detailing down times, error messages, HTTP status codes, and duration.

- **Monitor Management:** Full CRUD operations for monitors, pause/resume controls, and target grouping/tagging.

### 4.3 Public Status Page Engine

- **Dedicated Route:** Served at `/status` or root domain `/` when in status-page mode.

- **Read-Only Public Overview:** High-level system operational banner (e.g., "All Systems Operational", "Partial System Outage", "Major Outage").

- **90-Day Uptime Bar Charts:** Visual horizontal bar showing daily operational health color-coded (Green = Operational, Yellow = Degraded, Red = Down, Gray = No Data).

- **Service Grouping:** Categorize public monitors into collapsible sections (e.g., "APIs", "Web Sites", "Databases").

- **Privacy Control:** Toggle `is_public` boolean per monitor to keep internal infrastructure hidden while exposing customer-facing services.

### 4.4 Alerting & Notification Engine

- **Supported Channels:**
  - Discord Webhooks (Rich Embed cards with color-coded status banners).

  - Slack Webhooks (Formatted BlockKit payload).

  - Generic JSON Webhooks (POST payload with monitor metadata and error message).

- **State Transition Logic:**
  - Alerts are strictly triggered on state changes (`UP` $\rightarrow$ `DOWN` or `DOWN` $\rightarrow$ `UP`).

  - No continuous alert spam during extended outages.

  - "Resolved" alert includes total duration of downtime (e.g., "Service recovered after 4 minutes and 12 seconds").

## 5. Database Schema (SQLite)

The application uses **SQLite** in Write-Ahead Logging (WAL) mode (`PRAGMA journal_mode=WAL;`) for high-concurrency async reads and writes.

```sql
-- Monitors Configuration
CREATE TABLE IF NOT EXISTS monitors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    method TEXT NOT NULL DEFAULT 'GET',
    check_interval_secs INTEGER NOT NULL DEFAULT 60,
    timeout_secs INTEGER NOT NULL DEFAULT 5,
    max_retries INTEGER NOT NULL DEFAULT 3,
    expected_status_code INTEGER DEFAULT 200,
    keyword TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    is_public INTEGER NOT NULL DEFAULT 1,
    category TEXT DEFAULT 'General',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Real-time State (In-memory shadow mirror)
CREATE TABLE IF NOT EXISTS monitor_states (
    monitor_id TEXT PRIMARY KEY,
    current_status TEXT NOT NULL DEFAULT 'UP', -- 'UP', 'DEGRADED', 'DOWN', 'PAUSED'
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    last_check_at DATETIME,
    last_latency_ms INTEGER,
    last_error TEXT,
    last_state_change_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(monitor_id) REFERENCES monitors(id) ON DELETE CASCADE
);

-- Time-Series Metrics & Logs
CREATE TABLE IF NOT EXISTS check_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    monitor_id TEXT NOT NULL,
    status_code INTEGER,
    latency_ms INTEGER,
    is_up INTEGER NOT NULL,
    error_msg TEXT,
    checked_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(monitor_id) REFERENCES monitors(id) ON DELETE CASCADE
);

-- Notification Webhooks
CREATE TABLE IF NOT EXISTS notification_channels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    channel_type TEXT NOT NULL, -- 'DISCORD', 'SLACK', 'GENERIC_WEBHOOK'
    webhook_url TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Database Indices for Query Optimization
CREATE INDEX IF NOT EXISTS idx_logs_monitor_time ON check_logs(monitor_id, checked_at DESC);
CREATE INDEX IF NOT EXISTS idx_logs_time ON check_logs(checked_at DESC);
```

## 6. API Specification & Endpoints

### 6.1 Public Endpoints

- `GET /api/v1/public/status`
  - **Returns:** Public system overview, operational status, categorized public monitors, and 90-day daily uptime aggregates.

  - **Auth:** None required.

- `GET /api/v1/public/monitors/:id/history`
  - **Returns:** Response time graph data points for public monitor (last 24h).

### 6.2 Admin Endpoints (Authenticated)

- `GET /api/v1/monitors` - List all monitors (internal + public).

- `POST /api/v1/monitors` - Create a new monitor target.

- `PUT /api/v1/monitors/:id` - Update monitor configuration.

- `DELETE /api/v1/monitors/:id` - Delete monitor and purge historical logs.

- `POST /api/v1/monitors/:id/test` - Trigger an immediate manual ping test.

- `GET /api/v1/notifications` - Manage notification webhooks.

- `GET /api/v1/ws` - WebSocket connection for live dashboard streaming metrics.

## 7. State Machine & Notification Workflow

```
[ Scheduled Ping Tick ]
          |
          v
   Execute Request (Reqwest / TCP)
          |
   +------+------+
   |             |
[ Success ]  [ Failure ]
   |             |
   v             v
Consecutive   Increment Failure Counter
Failures = 0     |
   |             +--> Counter >= Threshold?
   |                     |          |
   |                    YES         NO
   |                     |          |
   |                     v          v
   |             State = DOWN   State = PENDING
   |                     |          |
   v                     v          v
State = UP      State Transition?  Log Metric
   |                     |
   +--------+------------+
            |
            v
   Did State Change?
    (UP -> DOWN or DOWN -> UP)
            |
     +------+------+
     |             |
    YES            NO
     |             |
     v             v
Send Alert      Done
(Discord/Slack)
```

## 8. Non-Functional Requirements (NFRs)

### 8.1 Performance & Footprint Targets

- **Idle Memory Footprint:** $< 15\text{ MB}$ RAM on Linux `x86_64`.

- **CPU Utilization:** $< 1\%$ average usage when monitoring 50 targets at 30-second intervals.

- **Binary Size:** $< 20\text{ MB}$ statically linked executable.

- **API Response Latency:** Sub-10ms $P_{99}$ response time for status page endpoints.

### 8.2 Reliability & Fault Tolerance

- **Panic Recovery:** Tokio tasks must catch exceptions/panics without bringing down the main process or web server.

- **Network Resilience:** DNS resolution errors or transient dropped packets must trigger retry logic before firing alert webhooks.

- **Zero Configuration:** Database files and tables are automatically bootstrapped via auto-migration on application launch.

## 9. Deployment Strategy & Docker Setup

KeepUp is delivered as both a bare-metal executable and a scratch/alpine Docker image.

### 9.1 Recommended `docker-compose.yml`

```yaml
version: "3.8"

services:
  keepup:
    image: ridloghfry/keepup:latest
    container_name: keepup
    restart: always
    ports:
      - "3000:3000"
    volumes:
      - ./data:/app/data
    environment:
      - PORT=3000
      - KEEPUP_DB_PATH=/app/data/keepup.db
      - RUST_LOG=info
```

## 10. Future Roadmap

- **Phase 1 (MVP):** HTTP/TCP pings, SQLite storage, Discord/Slack webhooks, embedded static UI, public status page.

- **Phase 2:** Multi-status pages, planned maintenance windows, multi-user role-based authentication.

- **Phase 3:** Distributed worker agents (pinging from multiple regional locations and calculating geographic quorum).
