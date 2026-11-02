# KeepUp Full Overhaul (Security, Logic Bugs, Lean Refactor & New Features) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a production-hardened, lean, single-binary KeepUp v0.2 that resolves all audited security vulnerabilities (`SEC-01`–`SEC-10`, `NV-01`–`NV-04`, Hardening), fixes all logic and SQLite scaling bugs (`#1`–`#11`), removes ~5,800+ lines of duplicate/legacy bloat (`#12`–`#14`), and implements all PRD-gap (`F1`–`F6`) and net-new (`N1`–`N6`) monitoring features.

**Architecture:** We execute in four dependency-ordered waves so legacy code is deleted before security/feature work touches those subsystems: (1) Lean consolidation of legacy `webhooks` into `notification_channels`, removal of `build.rs` minifier and duplicate static docs, `sqlx 0.8` + `argon2` upgrade, and one unified SQLite migration; (2) Auth/session/CORS/header hardening plus a unified `net_guard` SSRF & TLS signature verifier; (3) Batched `O(1)` index-friendly SQLite status queries with a 5s cache and a semaphore-bounded concurrent scheduler; (4) Flexible HTTP/JSON-Pointer/Degraded probes, TCP & DNS probes, Push heartbeats, Incident timelines, Prometheus `/metrics` + RSS feeds, Live SSE, JSON export/import, and an XSS-safe event-delegated frontend.

**Tech Stack:** Rust 2021, Axum 0.7 (`sse`, `macros`), Tokio 1.38 (trimmed features), SQLx 0.8 (`sqlite`, `runtime-tokio-rustls`, `chrono`, `migrate`), Reqwest 0.12 (`rustls-tls`, custom `Resolve` + `redirect::Policy`), Argon2 0.5, Rustls 0.23 / `tokio-rustls` 0.26 + `webpki-roots` 0.26, `rust-embed` 8.4, Vanilla JS + local Tailwind `/output.css`.

**Spec:** `/Users/driprogrammer7/.gemini/antigravity-cli/brain/e74ab4c3-8790-4707-93be-2f230b22b48c/keepup_audit_and_roadmap.md`

## Global Constraints

- Single-binary deployment preserved: all runtime dashboard/status assets embedded from `static/` via `rust-embed` (`#[folder = "static/"]`), zero external CDN `<script>` tags at runtime.
- Toolchain command (`$CARGO_TEST`): run `cargo test` natively if `cargo` is in `PATH`, otherwise via Docker container mounting workspace + cargo cache (`docker run --rm -v "$PWD":/app -v keepup-cargo-cache:/usr/local/cargo/registry -v keepup-target-cache:/app/target -w /app rust:1.83-alpine sh -c "apk add --no-cache musl-dev && cargo test"`).
- Outbound network policy: `169.254.169.254` (cloud metadata) and `0.0.0.0` are ALWAYS blocked; loopback (`127.0.0.0/8`, `::1`), RFC1918 (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), link-local (`169.254.0.0/16`, `fe80::/10`), CGNAT (`100.64.0.0/10`), and IPv6 ULA (`fc00::/7`) are blocked unless `ALLOW_PRIVATE_TARGETS=true`; cross-zone redirects from a public IP to a private/loopback IP are ALWAYS blocked even when `ALLOW_PRIVATE_TARGETS=true`.
- Every timestamp written to SQLite for cache/alert comparison (`domain_last_checked_at`, `ssl_last_checked_at`, `down_since`, `last_alerted_at`, `incidents.created_at`) uses RFC3339 UTC (`Utc::now().to_rfc3339()`), and SQLite time window predicates compare against `checked_at` directly (never `COALESCE(created_at, checked_at)`) or normalize both sides via `datetime(...)`.
- Frontend DOM safety: zero inline `onclick="..."` / `onchange="..."` attributes containing interpolated dynamic fields; all dynamic actions bind via `data-action` + `data-*` attributes read from `HTMLElement.dataset`.

## Review Focus

- `default` status page with explicit `status_page_monitors` assignments where every assigned monitor has `is_public = 0` or `is_active = 0` must return an empty `monitors` list (`[]`), NOT fall back to exposing all public monitors (pinned in Task 5).
- Public target returning `302 Location: http://127.0.0.1:...` or `http://169.254.169.254/...` must be rejected by `net_guard` even when `ALLOW_PRIVATE_TARGETS=true` (pinned in Task 4).
- Overnight maintenance window (`start_time = yesterday 23:00Z`, `end_time = today 01:00Z`) evaluated at `today 15:00 UTC` must return `is_monitor_in_maintenance == false` (pinned in Task 5).
- `insert_check_log` failure (e.g., closed/read-only DB or simulated error) during consecutive target failures must still advance `MonitorState.consecutive_failures` and return an `AlertEvent` once `max_retries` is reached (pinned in Task 6).
- Newly created `push://` monitor with zero `check_logs` rows must use `monitors.created_at` as its baseline and NOT trigger a `DOWN` alert before `check_interval_secs + grace_secs` has elapsed (pinned in Task 8).

---

### Task 1: Lean Build, Dependency Upgrade (`sqlx 0.8` + `argon2`) & Duplicate Asset Removal

**Files:**
- Modify: `Cargo.toml`
- Delete: `build.rs`
- Modify: `src/ui.rs:13-110`
- Delete: `static/docs.html`, `static/landing.html`, `static/landing.css`, `static/style.css`, `output.css`

**Interfaces:**
- Consumes: existing `Cargo.toml` and `src/ui.rs`
- Produces:
  - `Cargo.toml` with `sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "sqlite", "chrono", "migrate"] }`, `argon2 = "0.5"`, `axum = { version = "0.7", features = ["macros"] }` (`ws` removed), `tower-http = { version = "0.5", features = ["cors", "compression-gzip", "set-header"] }` (`trace` removed), `tokio` trimmed to `["rt-multi-thread", "macros", "net", "time", "sync", "signal", "fs", "io-util"]`, `rust-embed = { version = "8.4", features = ["axum"] }`, and `tower = { version = "0.5", features = ["util"] }` moved to `[dev-dependencies]`.
  - `src/ui.rs` using `#[folder = "static/"]`, requiring valid admin cookie for `"modal" | "modal.html"` (redirecting unauthenticated requests to `"/login.html"`), and removing `"docs"` / `"landing"` dead branches.

- [ ] **Step 1: Write failing UI asset & modal-auth unit test in `src/ui.rs`**

Add `#[cfg(test)] mod tests` in `src/ui.rs` asserting:
1. Unauthenticated `GET /modal.html` returns `303 See Other` / `307` redirect to `/login.html` (`Location: /login.html`).
2. `GET /output.css` returns `200 OK` with `Content-Type: text/css; charset=utf-8`.
3. `GET /docs.html` returns `404 Not Found`.

- [ ] **Step 2: Run test to verify RED**

Run: `$CARGO_TEST ui::tests`
Expected: FAIL (`GET /modal.html` currently returns `200 OK` without auth, `GET /docs.html` returns `200 OK`).

- [ ] **Step 3: Delete `build.rs` and duplicate assets, update `Cargo.toml` and `src/ui.rs`**

Delete `build.rs`, `static/docs.html`, `static/landing.html`, `static/landing.css`, `static/style.css`, and root `output.css` (keep `static/output.css`). Update `Cargo.toml` dependencies (fixing any `sqlx 0.8` trait imports if surfaced) and gate `"modal" | "modal.html"` behind JWT cookie verification in `src/ui.rs`. Attach `CompressionLayer::new()` in `src/main.rs`.

- [ ] **Step 4: Run full test suite to verify GREEN**

Run: `$CARGO_TEST`
Expected: PASS (all existing + new `ui::tests` green, 0 compilation errors on `sqlx 0.8`).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(lean): drop build.rs minifier and duplicate static docs, upgrade sqlx 0.8 and argon2"
```

---

### Task 2: Unified Schema Migration, Pool Hardening & Notification Consolidation

**Files:**
- Create: `migrations/20261103000000_keepup_v2_overhaul.sql`
- Modify: `src/db/mod.rs`
- Modify: `src/db/notifications.rs`
- Modify: `src/alerting/mod.rs`
- Modify: `src/api/admin.rs:470-676`
- Modify: `src/api/mod.rs`
- Modify: `src/main.rs`
- Delete: `src/notifier/mod.rs`, `src/notifier/discord.rs`, `src/notifier/slack.rs`, `src/notifier/generic.rs`, `src/db/webhooks.rs`, `src/api/webhooks.rs`, `src/api/status.rs`

**Interfaces:**
- Consumes: `migrations/*`
- Produces:
  - `db::init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error>` setting `.busy_timeout(Duration::from_secs(5))`, `.foreign_keys(true)`, `.pragma("cache_size", "-16000")`, and `0600` permissions on the SQLite file on `#[cfg(unix)]`.
  - Unified `db::notifications::NotificationChannel`:
    ```rust
    pub struct NotificationChannel {
        pub id: String,
        pub name: String,
        pub channel_type: String, // "discord" | "slack" | "telegram" | "webhook"
        pub url: String,
        pub is_default: bool,
        pub is_enabled: bool,
        pub use_default_template: bool,
        pub notify_on_resolve: bool,
        pub custom_template: Option<String>,
        pub created_at: String,
    }
    impl NotificationChannel {
        pub fn masked_view(&self) -> Self; // masks token/secret tail of `url` for GET responses
    }
    ```
  - Deduplicated `src/api/mod.rs`: one `fn build_v1_router(state: Arc<AppState>) -> Router<Arc<AppState>>` nested at `/api` and `/api/v1` (with `/webhooks` & `/webhooks/:id` mapped as thin aliases to `admin::list_channels` / `create_channel` / `update_channel` / `delete_channel` / `test_channel` or removed alongside legacy tests).
  - Migration `20261103000000_keepup_v2_overhaul.sql` migrating `webhooks` -> `notification_channels` and `monitor_webhooks` -> `monitor_notification_channels`, dropping `monitor_webhooks` and `webhooks`, adding new `monitors` columns (`last_ssl_alert_days`, `ssl_trusted`, `http_method`, `expected_status`, `request_headers`, `body_keyword`, `json_pointer`, `json_expected`, `degraded_latency_ms`, `push_token`, `grace_secs`, `dns_record_type`, `dns_expected`, `resend_interval_mins`, `last_alerted_at`), creating `incidents` and `incident_updates` tables, and adding `idx_spm_slug_lower`, `idx_mnc_channel`, `idx_monitors_push_token`, and `idx_incidents_slug_created`.

- [ ] **Step 1: Write failing migration & unified channel tests in `src/db/notifications.rs`**

Write unit tests asserting:
1. Running migrations creates `incidents`, `incident_updates`, new `monitors` columns, and `notification_channels` template/enabled/resolve columns, while `PRAGMA foreign_keys` is `1` on pooled connections from `init_pool`.
2. Creating/updating a `NotificationChannel` persists `is_enabled`, `use_default_template`, `notify_on_resolve`, and `custom_template`, and `masked_view()` redacts secret path/query segments of Discord/Slack/Telegram/Generic URLs (`SEC-04`).
3. `set_monitor_channels` returns `Err` when given a non-existent `channel_id` (foreign key enforced!).

- [ ] **Step 2: Run test to verify RED**

Run: `$CARGO_TEST db::notifications::tests`
Expected: FAIL (missing struct fields/columns and `PRAGMA foreign_keys` off on secondary pool connections).

- [ ] **Step 3: Implement unified migration, `init_pool` options, consolidated `alerting`/`notifications`, delete `src/notifier/*` & `webhooks.rs`, and deduplicate `src/api/mod.rs`**

Consolidate custom template rendering (`use_default_template == false` + `custom_template`) and `notify_on_resolve` checks into `src/alerting/mod.rs`, delete `src/notifier/`, `src/db/webhooks.rs`, `src/api/webhooks.rs`, and `src/api/status.rs`, return `masked_view()` on `GET /api/admin/channels` (preserving existing `url` on `PUT` when `payload.url` is `None` or masked), and collapse `src/api/mod.rs` to one router definition nested under `/api` and `/api/v1`.

- [ ] **Step 4: Run full test suite to verify GREEN**

Run: `$CARGO_TEST`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(notifications): consolidate legacy webhooks into notification_channels and apply v2 schema"
```

---

### Task 3: Auth, Argon2id, Persistent Secrets, Session Revocation, Proxy Rate-Limit & HTTP Security Headers

**Files:**
- Modify: `src/config.rs`
- Modify: `src/api/auth.rs`
- Modify: `src/api/admin.rs:426-467`
- Modify: `src/middleware/auth.rs`
- Modify: `src/middleware/rate_limit.rs`
- Modify: `src/error.rs`
- Modify: `src/main.rs`
- Modify: `Dockerfile`, `docker-compose.yml`

**Interfaces:**
- Consumes: `argon2`, `sqlx::SqlitePool`, `app_config` table
- Produces:
  - `Config`: adds `allow_private_targets: bool` (`ALLOW_PRIVATE_TARGETS`, default `false`), `trust_proxy_headers: bool` (`TRUST_PROXY_HEADERS`, default `false`), `cors_allowed_origins: Vec<String>` (`CORS_ALLOWED_ORIGINS`, default empty), `retention_days: u32` (`KEEPUP_RETENTION_DAYS`, default `90`), `custom_logo_url: Option<String>` (`CUSTOM_LOGO_URL`), `metrics_bearer_token: Option<String>` (`METRICS_BEARER_TOKEN`).
  - `auth::bootstrap_security_state(pool: &SqlitePool, config: &mut Config) -> anyhow::Result<u64>`:
    - Loads or persists random 32-byte hex `jwt_secret` in `app_config` when `JWT_SECRET` env var is unset or equals `"change-this-in-production-keepup-jwt-secret"`.
    - When `ADMIN_PASSWORD` is unset or `"admin"` and `app_config['admin_password']` is absent, generates a random 16-char bootstrap password, stores its Argon2id hash in `app_config`, and logs a prominent `tracing::warn!` once (except under `#[cfg(test)]` where explicit `Config` values in tests work deterministically).
    - Loads monotonic `session_version: u64` from `app_config['session_version']` (default `1`).
  - `AppState.session_version: Arc<AtomicU64>`
  - `Claims { pub sub: String, pub exp: usize, pub iat: usize, pub ver: u64 }`
  - `auth::create_jwt(username: &str, secret: &str, ver: u64)` & `auth::validate_jwt(token: &str, secret: &str, expected_ver: u64)`
  - `auth::hash_password(password: &str) -> String` (returns `"argon2:<phc>"`) & `auth::verify_password(input: &str, stored: &str) -> bool` (supports `"argon2:"`, legacy `"sha256:"`, and digest-based constant-time plaintext fallback).

- [ ] **Step 1: Write failing security tests in `src/api/auth.rs` and `src/api/admin.rs`**

Write tests asserting:
1. `hash_password` produces an `"argon2:$argon2id$..."` PHC string with unique random salts across two calls, `verify_password` verifies both `"argon2:"` and legacy `"sha256:"`, and `constant_time_compare` compares SHA-256 digests without early length return (`SEC-01`).
2. Changing password via `POST /api/admin/change-password` increments `session_version`, invalidates previously minted JWTs (`401 Unauthorized`), and sets a fresh valid `keepup_session` cookie (`SEC-06`).
3. `POST /api/auth/logout` increments `session_version` so the logged-out JWT is immediately rejected (`401 Unauthorized`) even if replayed as `Authorization: Bearer` (`SEC-06`).
4. Cross-origin `POST /api/auth/logout` with cookie authentication and mismatched `Origin: http://evil.localhost:3000` is rejected with `403 Forbidden` (`NV-03`), and `Origin: http://localhost:3000` receives NO `Access-Control-Allow-Origin` header by default (`SEC-04`).
5. `AppError::Database` returns `500` with generic `"Internal server error"` instead of raw SQLite schema details (`Hardening`), and responses carry `X-Frame-Options: DENY` and `X-Content-Type-Options: nosniff`.

- [ ] **Step 2: Run tests to verify RED**

Run: `$CARGO_TEST auth`
Expected: FAIL.

- [ ] **Step 3: Implement auth/session hardening, proxy-safe rate limiter, CSRF `Origin` check, security headers & Docker hardening**

Implement `bootstrap_security_state`, Argon2id hashing, `session_version` checking in `require_auth` / `me` / `static_handler` / `get_monitor_details`, rate limiting on `/login` and `/change-password` (using peer IP unless `TRUST_PROXY_HEADERS=true`, falling back gracefully in `oneshot` tests), `Origin` host match check in `require_auth` for cookie-authenticated non-`GET`/`HEAD`/`OPTIONS` requests, `DefaultBodyLimit::max(64 * 1024)`, response security headers (`X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: same-origin`, `Content-Security-Policy: default-src 'self'; img-src 'self' https: data:; style-src 'self' 'unsafe-inline'; script-src 'self'`), and remove hardcoded credentials/JWT secret from `docker-compose.yml` + `0555` root-owned binary in `Dockerfile`.

- [ ] **Step 4: Run full test suite to verify GREEN**

Run: `$CARGO_TEST`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "fix(security): enforce argon2id, persistent secrets, jwt revocation, origin checks, and safe cors/headers"
```

---

### Task 4: Outbound SSRF Guard (`net_guard`), Cryptographic TLS Handshake Verification & Safe Alert Escaping

**Files:**
- Create: `src/net_guard.rs`
- Modify: `src/main.rs`
- Modify: `src/checker/domain.rs`
- Modify: `src/checker/whois.rs`
- Modify: `src/checker/tls.rs`
- Modify: `src/checker/alerting.rs`
- Modify: `src/alerting/mod.rs`
- Modify: `src/api/admin.rs:608-643`

**Interfaces:**
- Consumes: `Config.allow_private_targets`, `reqwest::dns::Resolve`, `reqwest::redirect::Policy`, `tokio_rustls`, `webpki_roots`
- Produces:
  - `net_guard::is_allowed_ip(ip: IpAddr, allow_private: bool) -> bool`
  - `net_guard::resolve_checked(host: &str, port: u16, allow_private: bool) -> Result<Vec<SocketAddr>, NetGuardError>`
  - `net_guard::build_http_client(allow_private: bool, default_timeout: Duration) -> anyhow::Result<reqwest::Client>` (enforces custom DNS resolver checking every resolved IP + custom redirect policy capping at 5 hops, restricting to `http`/`https`, and blocking public-to-private cross-zone redirects even when `allow_private == true`).
  - `tls::SslExpiryInfo { pub host: String, pub expires_at: String, pub days_remaining: i64, pub is_trusted: bool }`
  - `tls::check_ssl_expiration(url: &str, timeout_secs: u64, allow_private: bool) -> Result<SslExpiryInfo, SslCheckError>` (verifies real TLS 1.2/1.3 handshake signatures via `ring::default_provider().signature_verification_algorithms`, connects via `net_guard::resolve_checked`, and sets `is_trusted` by verifying against `webpki_roots::TLS_SERVER_ROOTS`).

- [ ] **Step 1: Write failing unit tests for `net_guard`, `tls`, `whois`, and `alerting`**

Write tests asserting:
1. `is_allowed_ip` blocks `127.0.0.1`, `::1`, `10.0.0.1`, `172.16.0.1`, `192.168.1.1`, `100.64.0.1`, `fc00::1`, and `169.254.169.254` when `allow_private = false`, and STILL blocks `169.254.169.254` and `0.0.0.0` when `allow_private = true` (`SEC-02`).
2. `build_http_client(false, ...)` blocks an HTTP `302` redirect to `http://127.0.0.1:<port>/secret`, and `build_http_client(true, ...)` blocks a redirect hop to `http://169.254.169.254/latest/meta-data` (**Review Focus #2**).
3. `SignatureOnlyVerifier` in `src/checker/tls.rs` rejects a corrupted/forged `DigitallySignedStruct` handshake signature (`Err(RustlsError::InvalidCertificate(_))`) instead of returning unconditional `HandshakeSignatureValid::assertion()`, while marking a self-signed cert with a valid handshake signature as `is_trusted == false` (`SEC-07`).
4. `render_custom_template` JSON-escapes `"` and `\` inside `{{error}}` and `{{monitor_name}}` so `{"msg":"{{error}}"}` cannot inject extra JSON keys (`SEC-10`), Telegram uses `"parse_mode": "HTML"` with HTML-escaped `_`/`<`/`&` (`#6`), and `test_channel` failure messages report HTTP status code without echoing upstream non-2xx response bodies (`SEC-02`).

- [ ] **Step 2: Run tests to verify RED**

Run: `$CARGO_TEST net_guard checker::tls checker::alerting alerting`
Expected: FAIL.

- [ ] **Step 3: Implement `src/net_guard.rs`, update `tls.rs`, `whois.rs`, `domain.rs`, and `alerting`**

Wire `net_guard::build_http_client(config.allow_private_targets, Duration::from_secs(10))` into `AppState.http_client`, cap RDAP response reads in `domain.rs` to 1 MB (`resp.bytes()` / chunked cap), validate `resolve_checked(..., false)` on WHOIS referral targets in `whois.rs`, enforce real TLS 1.2/1.3 signature verification + `WebPkiServerVerifier` trust check in `tls.rs`, JSON-escape custom webhook template placeholders, switch Telegram to HTML parse mode, and strip raw response bodies from `test_channel` error returns.

- [ ] **Step 4: Run full test suite to verify GREEN**

Run: `$CARGO_TEST`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "fix(security): add net_guard ssrf protection, tls handshake signature verification, and safe alert escaping"
```

---

### Task 5: Public Status Privacy, Batched `O(1)` SQLite Queries, 5s Cache, Atomic Transactions & Timestamp Fixes

**Files:**
- Modify: `src/db/maintenance.rs`
- Modify: `src/db/monitors.rs`
- Modify: `src/db/checks.rs`
- Modify: `src/db/status_pages.rs`
- Modify: `src/api/public.rs`
- Modify: `src/api/admin.rs:19-175`
- Modify: `src/api/monitors.rs`
- Modify: `src/api/logs.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `idx_logs_monitor_time(monitor_id, checked_at DESC)`
- Produces:
  - `AppState.status_cache: Arc<RwLock<HashMap<String, (Instant, PublicStatus)>>>` (5-second TTL, invalidated whenever `reload_tx` fires or a monitor transitions state).
  - `PublicStatus` adds `pub custom_logo_url: Option<String>` (`F6`) and `pub incidents: Vec<PublicIncident>` (empty `Vec` until Task 8 populates `incidents`).
  - Batched helpers in `src/db/checks.rs`:
    - `get_batch_public_metrics(pool: &SqlitePool, monitor_ids: &[String]) -> Result<HashMap<String, BatchMonitorMetrics>, AppError>` executing 3 set-based queries over `checked_at >= datetime('now', '-7 days')` and `checked_at >= datetime('now', '-24 hours')` instead of `6N` per-monitor queries!

- [ ] **Step 1: Write failing tests for privacy leaks, overnight maintenance bug, inverted banner & oracle**

Write tests asserting:
1. Marking `"default"` status page `is_public = 0` causes both `GET /api/public/status` and `GET /api/public/status/DEFAULT` to return `404 Not Found` (`SEC-05`).
2. Assigning a private (`is_public = 0`) or paused (`is_active = 0`) monitor to `"default"` hides it from `GET /api/public/status`, AND when *all* assigned monitors on `"default"` are private/paused, `monitors` is `[]` rather than falling back to unassigned public monitors (**Review Focus #1**).
3. Overnight maintenance window (`yesterday 23:00Z` -> `today 01:00Z`) returns `is_monitor_in_maintenance == false` at `today 15:00 UTC` (**Review Focus #3**), and `get_public_maintenance_windows` excludes windows starting > 7 days in the future and filters `monitor_ids` strictly to the current status page's visible public monitors (`SEC-09`, `#3`).
4. A monitor currently `"UP"` with `uptime_24h == 99.95%` renders banner `"All Systems Operational"` while `"UP"` with `uptime_24h == 83.3%` (or `current_status == "DEGRADED"`) renders `"Degraded Performance"` / `"Partial System Outage"` (`#7`).
5. Unauthenticated `GET /api/admin/monitors/:id/details` returns `404` (not `401`) for private monitors, `404` for paused monitors, empty `channel_ids`, sanitized `error_message`, and non-`None` `tls_days_remaining` (`SEC-09`, `#5`).

- [ ] **Step 2: Run tests to verify RED**

Run: `$CARGO_TEST api::public db::maintenance api::admin`
Expected: FAIL.

- [ ] **Step 3: Fix timestamp SQL, remove `COALESCE` index bypasses, batch public status + 5s cache, and wrap DB updates in transactions**

Normalize `maintenance.rs` and `logs.rs` time comparisons via `datetime(...)`, rewrite `db/monitors.rs` and `db/checks.rs` queries against `checked_at` directly (populating `tls_days_remaining` from `ssl_days_remaining`), implement batched public status queries + 5s `status_cache` + `CUSTOM_LOGO_URL` in `src/api/public.rs`, sanitize unauthenticated `get_monitor_details` and cap `public_history` at `LIMIT 1000`, and wrap `create_monitor` / `update_monitor` / `update_maintenance_window` in atomic `pool.begin()` transactions propagating FK errors as `400 Bad Request`.

- [ ] **Step 4: Run full test suite to verify GREEN**

Run: `$CARGO_TEST`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "fix(db,public): enforce status page privacy, batch o(1) indexed queries, fix sqlite timestamps and transactions"
```

---

### Task 6: Concurrent Scheduler, RFC3339 12h Cache Fix, SSL Expiry Alerts (`F1`), Retention Pruning (`F2`), Repeat Reminders (`N5`) & Unified `ping_monitor`

**Files:**
- Modify: `src/api/dto.rs`
- Modify: `src/api/monitors.rs`
- Modify: `src/checker/scheduler.rs`
- Modify: `src/api/admin.rs:179-269`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `tls::check_ssl_expiration`, `db::monitors`, `alerting`
- Produces:
  - `AppState.monitor_states: Arc<tokio::sync::Mutex<HashMap<String, MonitorState>>>` shared between `run_scheduler` and `ping_monitor`.
  - Validated `CreateMonitorDto` (`check_interval_secs` `5..=86400`, `timeout_secs` `1..=60`, `max_retries` `1..=10`, supported schemes `http://`, `https://`, `tcp://`, `dns://`, `push://`).
  - Concurrent `run_scheduler` using `Semaphore::new(32)` + `in_flight: HashSet<String>`, hourly `prune_old_check_logs(&pool, config.retention_days)` (`DELETE FROM check_logs WHERE id IN (SELECT id FROM check_logs WHERE checked_at < datetime('now', '-' || ? || ' days') LIMIT 10000)` + `PRAGMA optimize`), SSL milestone alerts (`[30, 14, 7, 3, 1]` & `<= ssl_warning_days` tracked via `last_ssl_alert_days`), and repeat `DOWN` reminders when `resend_interval_mins > 0`.

- [ ] **Step 1: Write failing tests for DTO bounds, 12h cache, DB-error alert resilience, SSL expiry alert, repeat reminder & `ping_monitor`**

Write tests asserting:
1. `POST /api/monitors` rejects `timeout_secs: -1` and `check_interval_secs: 0` with `400 Bad Request` (`SEC-03`).
2. Calling `update_domain_expiry` / `update_ssl_expiry` writes an RFC3339 timestamp so `should_check_domain` and `should_check_ssl` immediately return `false` for the next 12 hours (`#1`).
3. When `insert_check_log` fails during consecutive failed checks, state evaluation still transitions `UP -> DOWN` at `max_retries` and emits the `DOWN` alert (**Review Focus #4**).
4. SSL expiry at `14` days remaining (`ssl_warning_days = 30`, `last_ssl_alert_days = None`) emits an SSL expiry alert and updates `last_ssl_alert_days = Some(14)`, suppressing duplicates on the next tick (`F1`).
5. Monitor in `DOWN` state with `resend_interval_mins = 30` and `last_alerted_at` 31 minutes ago emits a repeat reminder alert and updates `last_alerted_at` (`N5`).
6. `prune_old_check_logs` deletes rows older than `retention_days` while preserving recent rows (`F2`), and `ping_monitor` preserves existing `down_since` on an already-`DOWN` monitor and respects active maintenance (`#4`).

- [ ] **Step 2: Run tests to verify RED**

Run: `$CARGO_TEST checker::scheduler api::monitors`
Expected: FAIL.

- [ ] **Step 3: Implement DTO validation, RFC3339 timestamps, concurrent scheduler, SSL alerts, repeat reminders, retention pruning & state-safe `ping_monitor`**

Update `update_domain_expiry` and `update_ssl_expiry` to bind `Utc::now().to_rfc3339()`, refactor `run_scheduler` to spawn per-monitor tasks guarded by `Semaphore::new(32)` and `in_flight`, decouple state transition from `insert_check_log` errors, implement `evaluate_ssl_milestone` and `resend_interval_mins` reminder checks, add hourly `prune_old_check_logs`, and route `ping_monitor` through shared `AppState.monitor_states`.

- [ ] **Step 4: Run full test suite to verify GREEN**

Run: `$CARGO_TEST`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(scheduler): concurrent probes, fix 12h cache, ssl alerts, log pruning, repeat reminders, and safe ping"
```

---

### Task 7: Flexible HTTP Assertions (`F3`), JSON Pointer & Degraded Latency (`N4`), Async TCP Probe (`F4`) & DNS Drift Probe (`N6`)

**Files:**
- Modify: `src/checker/mod.rs`
- Modify: `src/checker/http.rs`
- Create: `src/checker/tcp.rs`
- Create: `src/checker/dns.rs`
- Modify: `src/checker/scheduler.rs`
- Modify: `src/api/dto.rs`
- Modify: `src/db/monitors.rs`

**Interfaces:**
- Consumes: `net_guard::resolve_checked`, `AppState.http_client`
- Produces:
  - `http::HttpProbeConfig<'a> { pub url: &'a str, pub method: &'a str, pub expected_status: &'a str, pub headers_json: Option<&'a str>, pub body_keyword: Option<&'a str>, pub json_pointer: Option<&'a str>, pub json_expected: Option<&'a str>, pub degraded_latency_ms: Option<i64>, pub timeout_secs: u64 }`
  - `http::CheckResult { pub status_code: Option<i32>, pub latency_ms: i64, pub is_up: bool, pub is_degraded: bool, pub error_msg: Option<String> }`
  - `tcp::ping_tcp(url: &str, timeout_secs: u64, allow_private: bool) -> CheckResult` (`tcp://host:port`)
  - `dns::ping_dns(url: &str, record_type: Option<&str>, expected: Option<&str>, timeout_secs: u64) -> CheckResult` (`dns://host`, supporting `A`/`AAAA` resolution match via `tokio::net::lookup_host` and UDP DNS `CNAME`/`TXT` or IP-set matching against `expected`).

- [ ] **Step 1: Write failing tests for HTTP assertions, JSON Pointer, Degraded latency, TCP probe & DNS probe**

Write tests using local ephemeral `tokio::net::TcpListener` / Axum test servers (`allow_private = true` in unit tests) asserting:
1. `POST` monitor with custom header `X-Health-Token: secret`, `expected_status: "200,202"`, `json_pointer: "/db/state"`, `json_expected: "ok"` passes when matched and fails with descriptive `error_msg` when JSON pointer value is `"down"` even on HTTP `200` (`F3`, `N4`).
2. Response body keyword check reads at most `64 * 1024` bytes and flags missing keyword as failure (`F3`), while response slower than `degraded_latency_ms` sets `is_up = true, is_degraded = true` and transitions monitor status to `"DEGRADED"` (`N4`).
3. `ping_tcp("tcp://127.0.0.1:<port>", ...)` succeeds when port is open, fails when closed, and rejects `127.0.0.1` when `allow_private = false` (`F4`).
4. `ping_dns("dns://localhost", Some("A"), Some("127.0.0.1"), ...)` succeeds when resolved IP matches `expected` and fails on drift (`N6`).

- [ ] **Step 2: Run tests to verify RED**

Run: `$CARGO_TEST checker::`
Expected: FAIL.

- [ ] **Step 3: Implement `HttpProbeConfig`, `checker/tcp.rs`, `checker/dns.rs`, and wire scheme dispatch into `scheduler.rs` & `ping_monitor`**

Dispatch by URL scheme (`http://` | `https://` -> `ping_http`, `tcp://` -> `ping_tcp`, `dns://` -> `ping_dns`) and persist/expose the new `Monitor` configuration fields across `db/monitors.rs` and `api/dto.rs`.

- [ ] **Step 4: Run full test suite to verify GREEN**

Run: `$CARGO_TEST`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(probes): add flexible http assertions, json pointer, degraded latency, tcp and dns probes"
```

---

### Task 8: Push/Heartbeat Monitoring (`N1`), Incident Timeline (`N2`), Prometheus `/metrics` & RSS Feed (`N3`), Live SSE (`F5`) & JSON Export/Import (`F6`)

**Files:**
- Create: `src/api/push.rs`
- Create: `src/db/incidents.rs`
- Create: `src/api/incidents.rs`
- Create: `src/api/observability.rs` (Prometheus `/metrics`, `/status/:slug/rss.xml`, SSE `/api/events`, `/api/admin/export` & `/api/admin/import`, batched `/api/admin/dashboard`)
- Modify: `src/checker/scheduler.rs`
- Modify: `src/api/public.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `incidents` & `incident_updates` tables, `monitors.push_token` & `grace_secs`
- Produces:
  - `AppState.events_tx: tokio::sync::broadcast::Sender<String>` (JSON SSE payloads for `check.completed` and `monitor.transition`).
  - `GET/POST /api/push/:token` (`Query { status: Option<String>, latency_ms: Option<i64>, msg: Option<String> }`) recording heartbeat and transitioning monitor `UP`/`DOWN`/`DEGRADED`; plus scheduler deadline check marking active `push://` monitors `DOWN` when `now - last_heartbeat_or_created_at > check_interval_secs + grace_secs`.
  - Admin Incident CRUD: `GET/POST /api/admin/incidents`, `POST /api/admin/incidents/:id/updates`, `DELETE /api/admin/incidents/:id`, auto-incident creation on `UP -> DOWN` and auto-resolution on `DOWN -> UP`, plus public `incidents` list in `GET /api/public/status` & `GET /api/public/status/:slug`.
  - `GET /metrics` (OpenMetrics text format; enforces `Authorization: Bearer <METRICS_BEARER_TOKEN>` when configured), `GET /status/:slug/rss.xml` & `GET /status/rss.xml` (XML-escaped RSS 2.0 feed of public incidents & maintenance windows).
  - `GET /api/events` (authenticated `axum::response::sse::Sse` stream), `GET /api/admin/dashboard` (1 batched response containing monitors + latest 30 ticks per monitor + summary metrics), and `GET /api/admin/export` + `POST /api/admin/import`.

- [ ] **Step 1: Write failing integration tests for Push heartbeat, Incident timeline, `/metrics`, RSS XML, SSE & Export/Import**

Write tests asserting:
1. Creating a `push://` monitor generates a 32-hex `push_token`, does NOT fire a missed-heartbeat `DOWN` alert before `check_interval_secs + grace_secs` (**Review Focus #5**), transitions `UP` on `GET /api/push/:token?latency_ms=42`, and transitions `DOWN` once overdue (`N1`).
2. Creating an incident (`Investigating`) and posting an update (`Identified` -> `Resolved`) exposes the timeline on `GET /api/public/status` only for status pages containing that monitor (or global incidents) (`N2`).
3. `GET /metrics` emits valid OpenMetrics lines (`keepup_monitor_up`, `keepup_monitor_latency_ms`, `keepup_ssl_days_remaining`) and enforces `METRICS_BEARER_TOKEN` when set; `GET /status/default/rss.xml` emits valid XML-escaped RSS 2.0 (`N3`).
4. `GET /api/events` requires `require_auth` and streams `text/event-stream` events when `events_tx` broadcasts (`F5`), and `GET /api/admin/export` -> `POST /api/admin/import` round-trips monitors and status pages cleanly without leaking unmasked channel secrets (`F6`).

- [ ] **Step 2: Run tests to verify RED**

Run: `$CARGO_TEST api::push api::incidents api::observability`
Expected: FAIL.

- [ ] **Step 3: Implement `push.rs`, `incidents.rs`, `observability.rs`, scheduler push-deadline & auto-incident hooks, and router mounts**

Implement all five capabilities cleanly with parameterized SQLx queries, XML/Prometheus label escaping, and `net_guard` URL validation on imported monitors.

- [ ] **Step 4: Run full test suite to verify GREEN**

Run: `$CARGO_TEST`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(api): add push heartbeats, incident timeline, prometheus metrics, rss feed, live sse, and json export/import"
```

---

### Task 9: XSS-Safe Frontend Overhaul, Local CSS, Live SSE Client & New Feature UI Controls

**Files:**
- Modify: `static/index.html`
- Modify: `static/login.html`
- Modify: `static/modal.html`
- Modify: `static/status.html`
- Modify: `static/app.js`
- Modify: `static/modal.js`
- Modify: `static/status.js`

**Interfaces:**
- Consumes: `/output.css`, `GET /api/events`, `GET /api/admin/dashboard`, `/api/admin/channels`, `/api/admin/incidents`, `/api/admin/export`, `/api/admin/import`, `PublicStatus { custom_logo_url, incidents, ... }`
- Produces:
  - Zero `<script src="https://cdn.tailwindcss.com">` tags across all HTML files (replaced by `<link rel="stylesheet" href="/output.css">`).
  - Zero inline `onclick="..."` / `onchange="..."` attributes interpolating dynamic data across `static/app.js`, `static/modal.js`, and `static/status.js` (`SEC-08`, fixing double-firing buttons).
  - Live `EventSource('/api/events')` + single `/api/admin/dashboard` fetch (paused when `document.hidden`, replacing the 203-request/5s polling storm `#8`).
  - Public `"Ping Now"` button removed from `static/status.html` (`#4`), custom logo (`custom_logo_url`), RSS feed link, and Incident Timeline rendered on `static/status.html`, and Admin UI controls wired for Monitor types (`HTTP/HTTPS`, `TCP`, `DNS`, `Push Heartbeat` with copyable push URL), HTTP assertions/JSON pointer/degraded latency/repeat reminder, Notification Channels custom template + resolve toggle, Incident management modal, and JSON Export/Import buttons.

- [ ] **Step 1: Write failing static bundle security & contract verification tests in `src/ui.rs`**

Add tests in `src/ui.rs` scanning embedded `Assets` (`index.html`, `login.html`, `modal.html`, `status.html`, `app.js`, `modal.js`, `status.js`) asserting:
1. None of the embedded HTML files contain `cdn.tailwindcss.com`.
2. Neither `app.js` nor `modal.js` contains inline `onclick="` with `${escapeHtml(` or `.replace(/'/g` (`SEC-08`).
3. `status.html` does not contain `triggerDetailPing` / `detail-modal-ping-btn` (`#4`), and contains the incident timeline container and RSS link (`N2`, `N3`).
4. `app.js` connects to `/api/events` via `EventSource` and uses `/api/admin/dashboard` instead of `/api/webhooks` (`#8`, `#12`, `F5`).

- [ ] **Step 2: Run tests to verify RED**

Run: `$CARGO_TEST ui::tests`
Expected: FAIL.

- [ ] **Step 3: Refactor `static/*.html` and `static/*.js` cleanly**

Replace Tailwind CDN scripts with `/output.css`, migrate all dynamic buttons in `app.js` and `modal.js` to `data-action` + `dataset` delegation, remove legacy webhooks UI in favor of unified Notification Channels, remove public `"Ping Now"` from `status.html`, wire `EventSource('/api/events')` + `/api/admin/dashboard`, and add concise UI panels for Push/TCP/DNS/HTTP-Assertions, Incidents, Custom Logo, RSS, and Export/Import.

- [ ] **Step 4: Run full test suite & release build to verify GREEN**

Run: `$CARGO_TEST && cargo build --release` (via `$CARGO_TEST` environment)
Expected: PASS (0 test failures, clean `--release` binary build).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(ui): eliminate dom xss and cdn scripts, connect live sse, and wire ui for incidents, push, tcp, dns, and export"
```
