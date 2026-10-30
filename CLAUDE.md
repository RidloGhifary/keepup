# Project Rules & Architecture Guidelines: KeepUp

## Tech Stack & Core Libraries

- Async Runtime: `tokio` (multi-thread)
- Web Framework: `axum` (v0.7+)
- Database: `sqlx` with SQLite (`runtime-tokio-rustls`, `sqlite`, `wal` mode)
- HTTP Client: `reqwest` (with rustls-tls)
- Embedded UI: `rust-embed`
- Logging & Diagnostics: `tracing` and `tracing-subscriber`
- Error Handling: `anyhow` for app logic, `thiserror` for library boundaries

## Coding & Architectural Conventions

1. **Module Structure:**
   - Keep `src/main.rs` minimal (setup logging, database pool, Tokio background task, and Axum router startup).
   - Separate concerns strictly: `db/`, `checker/`, `notifier/`, `api/`, `ui.rs`.
2. **State Management:**
   - Pass shared state in Axum using `AppState` wrapped in `Arc` (`Arc<AppState>`).
   - Store active monitor cancellation tokens or channels in memory using `dashmap` or `tokio::sync::RwLock`.
3. **Database Rules:**
   - ALWAYS enable WAL mode on SQLite startup (`PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;`).
   - Keep queries non-blocking using `sqlx`.
4. **Error Handling:**
   - API endpoints must return standard JSON error responses (`StatusCode` + JSON error payload).
   - Background Tokio tasks must log errors with `tracing::error!` and never panic.

## UI Design Guidelines: Clean Minimalist Aesthetics

### 1. Color Palette & Backgrounds

- **Dark Mode Background:** Crisp dark gray (`#000000` or `#0b0b0c`), NOT muddy blue-gray.
- **Card/Container Surface:** Translucent dark or white with backdrop blur (`bg-white/5` or `bg-neutral-900/60` + `backdrop-blur-xl`).
- **Hairline Borders:** Subtle 1px borders for depth (`border border-white/10` or `border-black/5`).
- **Status & Accent Colors:**
  - Status Green: `#30d158` (Operational)
  - Status Yellow: `#ffd60a` (Degraded)
  - Status Red: `#ff453a` (Down)
  - Primary Blue: `#0a84ff` (Primary actions/buttons)

### 2. Typography & Layout

- **Font Stack:** `system-ui, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", sans-serif`.
- **Corner Radii:** Continuous curvature corners — cards use `rounded-2xl` (16px–24px), buttons use `rounded-full` or `rounded-xl`.
- **Spacing:** Generous inner padding (`p-6` to `p-8`) with soft grid gaps (`gap-6`).

### 3. Depth & Visual Effects

- **Glassmorphic Cards:** Combine translucent background with subtle rim highlighting (`box-shadow: inset 0 1px 0 0 rgba(255, 255, 255, 0.1)`).
- **Status Indicators:** Glowing indicator dots using blurred backdrops (`shadow-[0_0_12px_rgba(48,209,88,0.4)]`).
- **Transitions:** Fast, spring-like easing (`transition-all duration-200 ease-out` or CSS cubic-bezier `cubic-bezier(0.16, 1, 0.3, 1)`).
