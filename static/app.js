// ── KeepUp Main Dashboard Controller (Clean Minimalist Aesthetics) ──────────────

// Global apiFetch wrapper fallback with 401 redirect loop guard
if (!window.apiFetch) {
  window.apiFetch = async function apiFetch(url, options = {}) {
    options.credentials = options.credentials || 'include';
    const res = await fetch(url, options);
    if (res.status === 401) {
      if (options.skipAuthRedirect || url.includes('/change-password')) {
        return res;
      }
      const isLoginPage =
        window.location.pathname.endsWith('/login.html') ||
        window.location.pathname === '/login';
      if (!isLoginPage) {
        window.location.href = '/login.html';
      }
      return res;
    }
    return res;
  };
}

// ── Dashboard Theme: Defaults to Dark ───────────────────────────────────────

function initDashboardTheme() {
  document.documentElement.classList.add('dark');
  document.documentElement.setAttribute('data-theme', 'dark');
  const themeMeta = document.querySelector('meta[name="theme-color"]');
  if (themeMeta) {
    themeMeta.setAttribute('content', '#0b0b0c');
  }
}

// Stub function to prevent errors from any legacy callers
function toggleDashboardTheme() {
  // Theme toggle is disabled on the dashboard (dark mode default)
}

initDashboardTheme();

// ── Admin Logout Handler ─────────────────────────────────────────────────────
async function handleLogout() {
  try {
    await apiFetch('/api/auth/logout', { method: 'POST' });
  } finally {
    window.location.replace('/login.html');
  }
}

window.allMonitors = [];
window.allWebhooks = [];
window.currentViewMode = localStorage.getItem('keepup_view_mode') || 'grid';
let statusMap = {};
let logsCache = {};
let isFetching = false;
let dashboardFetchController = null;

// ── Lightweight Batch DOM Insertion Helper ───────────────────────────────────

function batchInsertHTML(container, html) {
  if (!container) return;
  const fragment = document.createRange().createContextualFragment(html);
  container.replaceChildren(fragment);
}

// ── Dashboard Data Fetching ──────────────────────────────────────────────────

async function fetchDashboardData(showSpin = false) {
  if (isFetching) return;
  isFetching = true;

  if (dashboardFetchController) {
    dashboardFetchController.abort();
  }
  dashboardFetchController = new AbortController();
  const signal = dashboardFetchController.signal;

  const refreshIcon = document.getElementById('refresh-icon');
  if (showSpin && refreshIcon) refreshIcon.classList.add('animate-spin');

  if (currentAdminView === 'assets') {
    loadAssetExpiryData(showSpin);
  }

  try {
    const dashRes = await apiFetch('/api/admin/dashboard', { signal });

    // If unauthorized, do not render dashboard contents (redirect is underway)
    if (dashRes && dashRes.status === 401) {
      const isLoginPage =
        window.location.pathname.endsWith('/login.html') ||
        window.location.pathname === '/login';
      if (!isLoginPage) {
        window.location.href = '/login.html';
      }
      return;
    }

    if (dashRes && dashRes.ok) {
      const dash = await dashRes.json();
      const monitors = [];
      statusMap = {};

      (dash.monitors || []).forEach((item) => {
        const m = { ...item };
        monitors.push(m);
        // logs attached directly from single query
        logsCache[m.id] = (item.logs || []).slice().reverse();
        statusMap[m.id] = {
          id: m.id,
          name: m.name,
          url: m.url,
          current_status: m.last_status,
          last_latency_ms: (item.logs && item.logs.length > 0) ? item.logs[item.logs.length - 1].latency_ms : null,
          uptime_24h: 100.0,
        };
      });

      window.allMonitors = monitors;
      updateOverviewHero({
        status: dash.summary.down_count > 0 ? 'Major Outage' : (dash.summary.degraded_count > 0 ? 'Partial Outage' : 'All Systems Operational'),
        monitors: Object.values(statusMap),
      });

      renderMonitors();
    }
  } catch (err) {
    if (err.name === 'AbortError') return;
    console.error('Failed to fetch dashboard data:', err);
  } finally {
    isFetching = false;
    if (showSpin && refreshIcon) {
      setTimeout(() => refreshIcon.classList.remove('animate-spin'), 300);
    }
  }
}

// ── Recent Logs Batching for Historical Bar Chart ────────────────────────────

async function fetchRecentLogsForMonitors(monitors, signal) {
  const fetchPromises = monitors.map(async (m) => {
    try {
      const res = await apiFetch(`/api/v1/monitors/${m.id}/logs?limit=30`, signal ? { signal } : {});
      if (res && res.ok) {
        const logs = await res.json();
        // Reverse so chronological order from left to right
        logsCache[m.id] = logs.reverse();
      }
    } catch (e) {
      // Ignore background log fetch error
    }
  });

  await Promise.all(fetchPromises);
}

// ── Overview Hero & Statistics ───────────────────────────────────────────────

function updateOverviewHero(statusData) {
  const overallBadge = document.getElementById('overall-status-badge');
  const overallText = document.getElementById('overall-status-text');
  const overallPulse = document.getElementById('overall-status-pulse');

  const status = statusData.status || 'All Systems Operational';
  if (overallText) overallText.innerText = status;

  if (overallBadge && overallPulse) {
    if (status === 'All Systems Operational') {
      overallBadge.className =
        'inline-flex items-center gap-2 px-3.5 py-1.5 rounded-full text-xs font-medium bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 border border-emerald-500/20 shadow-sm shadow-emerald-500/10';
      overallPulse.className = 'h-2 w-2 rounded-full bg-[#30d158] animate-pulse';
    } else if (status.includes('Partial')) {
      overallBadge.className =
        'inline-flex items-center gap-2 px-3.5 py-1.5 rounded-full text-xs font-medium bg-amber-500/15 text-amber-700 dark:text-amber-400 border border-amber-500/20 shadow-sm shadow-amber-500/10';
      overallPulse.className = 'h-2 w-2 rounded-full bg-[#ffd60a] animate-pulse';
    } else {
      overallBadge.className =
        'inline-flex items-center gap-2 px-3.5 py-1.5 rounded-full text-xs font-medium bg-rose-500/15 text-rose-700 dark:text-rose-400 border border-rose-500/20 shadow-sm shadow-rose-500/10';
      overallPulse.className = 'h-2 w-2 rounded-full bg-[#ff453a]';
    }
  }

  // 1. Active Monitors Count
  const activeCount = window.allMonitors.filter((m) => m.is_active === 1).length;
  const statActive = document.getElementById('hero-stat-active');
  if (statActive) {
    statActive.innerText = `${activeCount} / ${window.allMonitors.length}`;
  }

  // 2. 24h Average Uptime %
  const publicMonitors = statusData.monitors || [];
  let avgUptime = 100.0;
  if (publicMonitors.length > 0) {
    const sumUptime = publicMonitors.reduce((acc, m) => acc + (m.uptime_24h || 100), 0);
    avgUptime = sumUptime / publicMonitors.length;
  }
  const statUptime = document.getElementById('hero-stat-uptime');
  if (statUptime) {
    statUptime.innerText = `${avgUptime.toFixed(2)}%`;
  }

  // 3. Average Response Time (ms)
  let latencies = [];
  window.allMonitors.forEach((m) => {
    const live = statusMap[m.id];
    if (live && live.last_latency_ms !== null && live.last_latency_ms !== undefined) {
      latencies.push(live.last_latency_ms);
    }
  });

  const avgLatency =
    latencies.length > 0
      ? Math.round(latencies.reduce((a, b) => a + b, 0) / latencies.length)
      : 0;

  const statLatency = document.getElementById('hero-stat-latency');
  if (statLatency) {
    statLatency.innerText = latencies.length > 0 ? `${avgLatency} ms` : '—';
  }
}

// ── Rendering Monitors (Card Grid vs Compact List) ───────────────────────────

function renderMonitors() {
  const container = document.getElementById('monitors-container');
  if (!container) return;

  const searchInput = document.getElementById('search-input');
  const query = searchInput ? searchInput.value.toLowerCase().trim() : '';

  const filtered = window.allMonitors.filter(
    (m) =>
      m.name.toLowerCase().includes(query) || m.url.toLowerCase().includes(query)
  );

  const countBadge = document.getElementById('monitors-count-badge');
  if (countBadge) countBadge.innerText = `${filtered.length} target${filtered.length === 1 ? '' : 's'}`;

  if (filtered.length === 0) {
    container.className = 'w-full';
    batchInsertHTML(
      container,
      `
      <div class="glass-card rounded-2xl p-12 text-center text-white/50 space-y-3">
        <svg class="w-10 h-10 mx-auto text-white/30" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
        </svg>
        <p class="text-sm font-medium text-white/70">${query ? 'No matching targets found' : 'No monitors configured yet'}</p>
        <p class="text-xs text-white/40">${query ? 'Try searching for a different keyword' : 'Click "+ Add Monitor" to create your first health check target'}</p>
      </div>
    `
    );
    return;
  }

  if (window.currentViewMode === 'grid') {
    container.className = 'grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4';
    batchInsertHTML(container, filtered.map((m) => renderMonitorCard(m)).join(''));
  } else {
    container.className = 'space-y-2.5';
    batchInsertHTML(container, filtered.map((m) => renderMonitorListRow(m)).join(''));
  }
}

// ── Domain & SSL Expiration Badges ──────────────────────────────────────────

function renderDomainExpiryBadge(m) {
  if (!m.check_domain_expiry || m.domain_days_remaining === null || m.domain_days_remaining === undefined) {
    return '';
  }
  const days = m.domain_days_remaining;
  const warningDays = m.domain_warning_days || 30;

  if (days <= 0) {
    return `<span class="px-2 py-0.5 rounded-full text-[10px] font-mono font-medium border bg-rose-500/15 text-rose-600 dark:text-rose-400 border-rose-500/20" title="Domain registration has expired">🔴 Domain Expired</span>`;
  } else if (days <= warningDays) {
    return `<span class="px-2 py-0.5 rounded-full text-[10px] font-mono font-medium border bg-amber-500/15 text-amber-600 dark:text-amber-400 border-amber-500/20" title="Domain registration expiring soon (${days} days remaining)">⚠️ ${days}d domain left</span>`;
  } else {
    return `<span class="px-2 py-0.5 rounded-full text-[10px] font-mono font-medium border bg-black/5 dark:bg-white/5 text-neutral-600 dark:text-white/60 border-black/5 dark:border-white/10" title="Domain registration healthy (${days} days remaining)">🌐 ${days}d domain left</span>`;
  }
}

function renderSslExpiryBadge(m) {
  if (!m.check_ssl || m.ssl_days_remaining === null || m.ssl_days_remaining === undefined) {
    return '';
  }
  const days = m.ssl_days_remaining;
  const warningDays = m.ssl_warning_days || 30;

  if (days <= 0) {
    return `<span class="px-2 py-0.5 rounded-full text-[10px] font-mono font-medium border bg-rose-500/15 text-rose-600 dark:text-rose-400 border-rose-500/20" title="TLS/SSL certificate has expired">🔴 SSL Expired</span>`;
  } else if (days <= warningDays) {
    return `<span class="px-2 py-0.5 rounded-full text-[10px] font-mono font-medium border bg-amber-500/15 text-amber-600 dark:text-amber-400 border-amber-500/20" title="TLS/SSL certificate expiring soon (${days} days remaining)">🔒 ${days}d SSL left</span>`;
  } else {
    return `<span class="px-2 py-0.5 rounded-full text-[10px] font-mono font-medium border bg-black/5 dark:bg-white/5 text-neutral-600 dark:text-white/60 border-black/5 dark:border-white/10" title="TLS/SSL certificate healthy (${days} days remaining)">🔒 ${days}d SSL left</span>`;
  }
}

// ── Single Monitor Card (Grid View) ──────────────────────────────────────────

function renderMonitorCard(m) {
  const live = statusMap[m.id];
  const isUp = live ? live.current_status === 'UP' : true;
  const isPaused = m.is_active === 0;

  let statusColor = '#30d158'; // Status Green
  let statusBadgeBg = 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 border border-emerald-500/20';
  let statusLabel = 'OPERATIONAL';

  if (isPaused) {
    statusColor = '#8e8e93'; // Neutral Gray
    statusBadgeBg = 'bg-neutral-500/15 text-neutral-600 dark:text-neutral-400 border border-neutral-500/20';
    statusLabel = 'PAUSED';
  } else if (live && live.current_status === 'MAINTENANCE') {
    statusColor = '#c084fc'; // Maintenance Purple
    statusBadgeBg = 'bg-purple-500/15 text-purple-700 dark:text-purple-400 border border-purple-500/20';
    statusLabel = 'MAINTENANCE';
  } else if (!isUp) {
    statusColor = '#ff453a'; // Outage Red
    statusBadgeBg = 'bg-rose-500/15 text-rose-700 dark:text-rose-400 border border-rose-500/20';
    statusLabel = 'DOWN';
  } else if (live && (live.current_status === 'DEGRADED' || (live.uptime_24h !== undefined && live.uptime_24h < 95.0))) {
    statusColor = '#ffd60a'; // Degraded Yellow
    statusBadgeBg = 'bg-amber-500/15 text-amber-700 dark:text-amber-400 border border-amber-500/20';
    statusLabel = 'DEGRADED';
  }

  const latencyText =
    live && live.last_latency_ms !== null && live.last_latency_ms !== undefined
      ? `${live.last_latency_ms} ms`
      : '—';

  const uptimeText =
    live && live.uptime_24h !== undefined ? `${live.uptime_24h.toFixed(1)}%` : '100%';

  const logs = logsCache[m.id] || [];
  const barsHtml = renderUptimeTickBars(logs);

  return `
    <div data-action="open-detail" data-id="${m.id}" class="glass-card rounded-2xl p-5 shadow-xl transition-all duration-200 hover:border-black/15 dark:hover:border-white/20 flex flex-col justify-between space-y-4 cursor-pointer">
      <!-- Card Header -->
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <span class="relative flex h-2.5 w-2.5">
              ${!isPaused && isUp ? `<span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-[${statusColor}] opacity-75"></span>` : ''}
              <span class="relative inline-flex rounded-full h-2.5 w-2.5" style="background-color: ${statusColor}"></span>
            </span>
            <h3 class="font-semibold text-neutral-900 dark:text-white text-sm sm:text-base truncate tracking-tight">${escapeHtml(m.name)}</h3>
          </div>
          <p class="text-xs text-neutral-500 dark:text-white/40 truncate font-mono mt-0.5">${escapeHtml(m.url)}</p>
        </div>

        <div class="flex items-center gap-1.5 shrink-0">
          ${renderSslExpiryBadge(m)}
          ${renderDomainExpiryBadge(m)}
          <span class="px-2 py-0.5 rounded-full text-[10px] font-medium border uppercase tracking-wider ${statusBadgeBg}">
            ${statusLabel}
          </span>
        </div>
      </div>

      <!-- Metrics Row -->
      <div class="grid grid-cols-3 gap-2 py-2 border-y border-black/5 dark:border-white/5 text-center">
        <div>
          <div class="text-[10px] uppercase tracking-wider text-neutral-500 dark:text-white/40">Latency</div>
          <div class="text-sm font-semibold text-neutral-900 dark:text-white font-mono mt-0.5">${latencyText}</div>
        </div>
        <div>
          <div class="text-[10px] uppercase tracking-wider text-neutral-500 dark:text-white/40">24h Uptime</div>
          <div class="text-sm font-semibold text-neutral-900 dark:text-white font-mono mt-0.5">${uptimeText}</div>
        </div>
        <div>
          <div class="text-[10px] uppercase tracking-wider text-neutral-500 dark:text-white/40">Interval</div>
          <div class="text-sm font-semibold text-neutral-700 dark:text-white/80 font-mono mt-0.5">${m.check_interval_secs}s</div>
        </div>
      </div>

      <!-- Historical Bar Chart (30 Ticks) -->
      <div class="space-y-1.5">
        <div class="flex items-center justify-between text-[10px] text-neutral-500 dark:text-white/40 font-mono">
          <span>Recent Checks</span>
          <span>${logs.length > 0 ? `${logs.length} ticks` : 'No logs'}</span>
        </div>
        <div class="flex items-center gap-[2px] h-6 w-full p-1 bg-black/5 dark:bg-black/30 rounded-lg border border-black/5 dark:border-white/5">
          ${barsHtml}
        </div>
      </div>

      <!-- Card Footer Actions -->
      <div class="flex items-center justify-between pt-1 text-xs">
        <span class="text-[11px] text-neutral-500 dark:text-white/40 font-mono">
          ${m.is_public === 1 ? '🌐 Public' : '🔒 Private'}
        </span>
        <div class="flex items-center gap-1.5">
          <button data-action="toggle-active" data-id="${m.id}" data-active="${m.is_active === 1}" class="p-1.5 rounded-lg text-neutral-500 hover:text-neutral-900 dark:text-white/60 dark:hover:text-white hover:bg-black/5 dark:hover:bg-white/10 transition" title="${m.is_active === 1 ? 'Pause Monitor' : 'Resume Monitor'}">
            ${
              m.is_active === 1
                ? `<svg class="w-4 h-4 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 9v6m4-6v6m7-3a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>`
                : `<svg class="w-4 h-4 text-[#30d158] pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z"/><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>`
            }
          </button>
          <button data-action="open-badge" data-id="${m.id}" data-name="${escapeHtml(m.name)}" class="p-1.5 text-neutral-400 hover:text-emerald-400 hover:bg-white/5 rounded-md transition" title="Get Status Badge">
            <svg class="w-4 h-4 pointer-events-none" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h10v10M7 17L17 7"/>
            </svg>
          </button>
          <button data-action="edit-monitor" data-id="${m.id}" class="px-2.5 py-1 rounded-lg bg-black/5 hover:bg-black/10 dark:bg-white/5 dark:hover:bg-white/10 text-neutral-700 hover:text-neutral-900 dark:text-white/80 dark:hover:text-white font-medium border border-black/5 dark:border-white/10 transition" title="Edit Target">
            Edit
          </button>
          <button data-action="view-logs" data-id="${m.id}" data-name="${escapeHtml(m.name)}" data-url="${escapeHtml(m.url)}" class="px-2.5 py-1 rounded-lg bg-black/5 hover:bg-black/10 dark:bg-white/5 dark:hover:bg-white/10 text-neutral-700 hover:text-neutral-900 dark:text-white/80 dark:hover:text-white font-medium border border-black/5 dark:border-white/10 transition" title="View History Logs">
            Logs
          </button>
          <button data-action="delete-monitor" data-id="${m.id}" data-name="${escapeHtml(m.name)}" class="p-1.5 rounded-lg text-neutral-400 hover:text-[#ff453a] hover:bg-[#ff453a]/10 transition" title="Delete Target">
            <svg class="w-4 h-4 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
            </svg>
          </button>
        </div>
      </div>
    </div>
  `;
}

// ── Single Monitor List Row (Compact View) ───────────────────────────────────

function renderMonitorListRow(m) {
  const live = statusMap[m.id];
  const isUp = live ? live.current_status === 'UP' : true;
  const isPaused = m.is_active === 0;

  let statusColor = '#30d158';
  let statusBadgeBg = 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 border border-emerald-500/20';
  let statusLabel = 'UP';

  if (isPaused) {
    statusColor = '#8e8e93';
    statusBadgeBg = 'bg-neutral-500/15 text-neutral-600 dark:text-neutral-400 border border-neutral-500/20';
    statusLabel = 'PAUSED';
  } else if (live && live.current_status === 'MAINTENANCE') {
    statusColor = '#c084fc';
    statusBadgeBg = 'bg-purple-500/15 text-purple-700 dark:text-purple-400 border border-purple-500/20';
    statusLabel = 'MAINTENANCE';
  } else if (!isUp) {
    statusColor = '#ff453a';
    statusBadgeBg = 'bg-rose-500/15 text-rose-700 dark:text-rose-400 border border-rose-500/20';
    statusLabel = 'DOWN';
  } else if (live && (live.current_status === 'DEGRADED' || (live.uptime_24h !== undefined && live.uptime_24h < 95.0))) {
    statusColor = '#ffd60a';
    statusBadgeBg = 'bg-amber-500/15 text-amber-700 dark:text-amber-400 border border-amber-500/20';
    statusLabel = 'DEGRADED';
  }

  const latencyText =
    live && live.last_latency_ms !== null && live.last_latency_ms !== undefined
      ? `${live.last_latency_ms} ms`
      : '—';

  return `
    <div data-action="open-detail" data-id="${m.id}" class="glass-card rounded-2xl p-4 shadow-lg flex flex-col sm:flex-row sm:items-center justify-between gap-3 hover:border-black/15 dark:hover:border-white/20 transition cursor-pointer">
      <div class="flex items-center gap-3 min-w-0">
        <span class="relative flex h-2.5 w-2.5 shrink-0">
          <span class="relative inline-flex rounded-full h-2.5 w-2.5" style="background-color: ${statusColor}"></span>
        </span>
        <div class="min-w-0">
          <div class="flex items-center gap-2">
            <span class="font-semibold text-neutral-900 dark:text-white text-sm truncate">${escapeHtml(m.name)}</span>
            <span class="px-2 py-0.5 rounded-full text-[10px] font-medium border uppercase ${statusBadgeBg}">${statusLabel}</span>
            ${renderSslExpiryBadge(m)}
            ${renderDomainExpiryBadge(m)}
          </div>
          <div class="text-xs text-neutral-500 dark:text-white/40 truncate font-mono">${escapeHtml(m.url)}</div>
        </div>
      </div>

      <div class="flex items-center justify-between sm:justify-end gap-4 font-mono text-xs">
        <div class="text-right">
          <span class="text-neutral-900 dark:text-white font-medium">${latencyText}</span>
          <span class="text-neutral-500 dark:text-white/40 text-[10px] ml-1">(${m.check_interval_secs}s)</span>
        </div>

        <div class="flex items-center gap-1.5 shrink-0">
          <button data-action="toggle-active" data-id="${m.id}" data-active="${m.is_active === 1}" class="p-1.5 rounded-lg text-neutral-500 hover:text-neutral-900 dark:text-white/60 dark:hover:text-white hover:bg-black/5 dark:hover:bg-white/10 transition" title="Toggle Pause">
            ${m.is_active === 1 ? '⏸' : '▶'}
          </button>
          <button data-action="open-badge" data-id="${m.id}" data-name="${escapeHtml(m.name)}" class="p-1.5 text-neutral-400 hover:text-emerald-400 hover:bg-white/5 rounded-md transition" title="Get Status Badge">
            <svg class="w-4 h-4 pointer-events-none" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h10v10M7 17L17 7"/>
            </svg>
          </button>
          <button data-action="edit-monitor" data-id="${m.id}" class="px-2.5 py-1 rounded-lg bg-black/5 hover:bg-black/10 dark:bg-white/5 dark:hover:bg-white/10 text-neutral-700 hover:text-neutral-900 dark:text-white/80 dark:hover:text-white border border-black/5 dark:border-white/10 font-medium transition">Edit</button>
          <button data-action="view-logs" data-id="${m.id}" data-name="${escapeHtml(m.name)}" data-url="${escapeHtml(m.url)}" class="px-2.5 py-1 rounded-lg bg-black/5 hover:bg-black/10 dark:bg-white/5 dark:hover:bg-white/10 text-neutral-700 hover:text-neutral-900 dark:text-white/80 dark:hover:text-white border border-black/5 dark:border-white/10 font-medium transition">Logs</button>
          <button data-action="delete-monitor" data-id="${m.id}" data-name="${escapeHtml(m.name)}" class="p-1.5 text-neutral-400 hover:text-[#ff453a] hover:bg-[#ff453a]/10 rounded-lg transition">✕</button>
        </div>
      </div>
    </div>
  `;
}

// ── Historical 30-Tick Bar Generator ─────────────────────────────────────────

function renderUptimeTickBars(logs) {
  const totalTicks = 30;
  const recent = logs.slice(-totalTicks);
  const emptyCount = Math.max(0, totalTicks - recent.length);

  let html = '';

  // Placeholder ticks if fewer than 30 logs available
  for (let i = 0; i < emptyCount; i++) {
    html += `<div class="flex-1 h-full rounded-[1.5px] bg-black/10 dark:bg-white/5" title="No check record"></div>`;
  }

  // Render actual tick logs
  recent.forEach((log) => {
    const isUp = log.is_up === 1;
    const color = isUp ? 'bg-[#30d158]' : 'bg-[#ff453a]';
    const latency = log.latency_ms !== null ? `${log.latency_ms} ms` : 'err';
    const status = isUp ? 'UP' : 'DOWN';
    const time = formatDate(log.checked_at);

    html += `<div class="flex-1 h-full rounded-[1.5px] ${color} hover:opacity-80 transition cursor-pointer" title="${status} • ${latency} • ${time}"></div>`;
  });

  return html;
}

// ── Webhooks Preview Section ─────────────────────────────────────────────────

function renderWebhookItem(w) {
  const isEnabled = w.is_enabled === 1 || w.is_enabled === true;
  const providerColor =
    w.provider.toLowerCase() === 'discord'
      ? 'text-[#5865F2] border-[#5865F2]/30 bg-[#5865F2]/10'
      : w.provider.toLowerCase() === 'slack'
      ? 'text-[#ECB22E] border-[#ECB22E]/30 bg-[#ECB22E]/10'
      : 'text-[#30d158] border-[#30d158]/30 bg-[#30d158]/10';

  return `
    <div class="glass-card rounded-2xl p-4 flex items-center justify-between gap-3 shadow-lg hover:border-black/15 dark:hover:border-white/20 transition">
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2">
          <span class="font-semibold text-neutral-900 dark:text-white text-xs truncate">${escapeHtml(w.name)}</span>
          <span class="px-2 py-0.5 rounded-full text-[9px] uppercase font-mono border ${providerColor}">${escapeHtml(w.provider)}</span>
          <span class="px-1.5 py-0.5 rounded text-[9px] uppercase font-mono bg-black/5 dark:bg-white/5 text-neutral-600 dark:text-neutral-400 border border-black/5 dark:border-white/5">${w.use_default_template !== 0 && w.use_default_template !== false ? 'Default' : 'Custom'}</span>
          <span class="px-1.5 py-0.5 rounded text-[9px] uppercase font-mono ${w.notify_on_resolve !== 0 && w.notify_on_resolve !== false ? 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 border border-emerald-500/20' : 'bg-black/5 dark:bg-white/5 text-neutral-500 dark:text-neutral-400 border border-black/5 dark:border-white/5'}" title="Resolve Notifications">${w.notify_on_resolve !== 0 && w.notify_on_resolve !== false ? 'Resolve: ON' : 'Resolve: OFF'}</span>
        </div>
        <p class="text-[11px] text-neutral-500 dark:text-white/40 truncate font-mono mt-0.5">${escapeHtml(w.url)}</p>
      </div>

      <div class="flex items-center gap-2 shrink-0">
        <button data-action="test-webhook" data-id="${w.id}" data-name="${escapeHtml(w.name)}" class="px-2.5 py-1 text-xs font-medium rounded-lg bg-black/5 hover:bg-black/10 dark:bg-white/5 dark:hover:bg-white/10 text-neutral-700 hover:text-neutral-900 dark:text-white/80 dark:hover:text-white border border-black/5 dark:border-white/10 transition" title="Test Notification">
          Test
        </button>
        <!-- Smooth toggle -->
        <label class="relative inline-flex items-center cursor-pointer">
          <input type="checkbox" ${isEnabled ? 'checked' : ''} data-action="toggle-webhook" data-id="${w.id}" data-enabled="${isEnabled}" class="sr-only peer">
          <div class="w-9 h-5 bg-neutral-300 dark:bg-neutral-700 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-[#30d158]"></div>
        </label>
      </div>
    </div>
  `;
}

function renderWebhooksSection(webhooks) {
  const container = document.getElementById('webhooks-preview-list');
  if (!container) return;

  if (webhooks.length === 0) {
    batchInsertHTML(
      container,
      `
      <div class="col-span-full p-6 text-center text-neutral-500 dark:text-white/40 text-xs glass-card rounded-xl">
        No notification webhooks registered. Click "+ Add Webhook" to receive instant alert cards.
      </div>
    `
    );
    return;
  }

  batchInsertHTML(container, webhooks.map((w) => renderWebhookItem(w)).join(''));
}

// ── Interactive Actions (Pause/Resume, Webhook Toggle, Direct Test) ───────────

async function toggleMonitorActive(id, currentActive) {
  const newActive = !currentActive;
  try {
    const res = await apiFetch(`/api/monitors/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ is_active: newActive }),
    });

    if (res && res.ok) {
      showToast({
        title: 'Monitor Updated',
        message: newActive ? 'Target resumed (checks active)' : 'Target paused (checks stopped)',
        type: 'info',
      });
      fetchDashboardData();
    } else if (res && res.status !== 401 && res.status !== 429) {
      showToast({
        title: 'Update Failed',
        message: 'Failed to update monitor status',
        type: 'error',
      });
    }
  } catch (err) {
    // Already handled by apiFetch toast
  }
}

async function toggleWebhookActive(id, currentEnabled) {
  const newEnabled = !currentEnabled;
  try {
    const res = await apiFetch(`/api/admin/channels/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ is_enabled: newEnabled }),
    });

    if (res && res.ok) {
      showToast({
        title: 'Webhook Updated',
        message: newEnabled ? 'Webhook alert channel enabled' : 'Webhook alert channel disabled',
        type: 'info',
      });
      fetchDashboardData();
    } else if (res && res.status !== 401 && res.status !== 429) {
      showToast({
        title: 'Update Failed',
        message: 'Failed to toggle webhook state',
        type: 'error',
      });
    }
  } catch (err) {
    // Already handled by apiFetch toast
  }
}

async function testWebhookDirect(id, name) {
  showToast({
    title: 'Testing Webhook',
    message: `Dispatching test alert to "${name}"...`,
    type: 'info',
  });
  try {
    const res = await apiFetch(`/api/admin/channels/${id}/test`, { method: 'POST' });
    if (res) {
      const data = await res.json().catch(() => ({}));
      if (res.ok) {
        showToast({
          title: 'Alert Delivered',
          message: data.message || 'Test alert delivered successfully!',
          type: 'success',
        });
      } else if (res.status !== 401 && res.status !== 429) {
        showToast({
          title: 'Delivery Failed',
          message: data.error || 'Test alert delivery failed',
          type: 'error',
        });
      }
    }
  } catch (err) {
    // Already handled by apiFetch toast
  }
}

// ── View Mode Switcher (Grid vs List) ────────────────────────────────────────

function setViewMode(mode) {
  window.currentViewMode = mode;
  localStorage.setItem('keepup_view_mode', mode);

  const gridBtn = document.getElementById('view-grid-btn');
  const listBtn = document.getElementById('view-list-btn');

  if (gridBtn && listBtn) {
    if (mode === 'grid') {
      gridBtn.className = 'p-1.5 rounded-lg text-white bg-white/10 transition';
      listBtn.className = 'p-1.5 rounded-lg text-white/40 hover:text-white transition';
    } else {
      listBtn.className = 'p-1.5 rounded-lg text-white bg-white/10 transition';
      gridBtn.className = 'p-1.5 rounded-lg text-white/40 hover:text-white transition';
    }
  }

  renderMonitors();
}

function filterMonitors() {
  renderMonitors();
}

// ── Global Helper Utilities ──────────────────────────────────────────────────

function escapeHtml(str) {
  if (!str) return '';
  return String(str).replace(
    /[&<>"']/g,
    (s) =>
      ({
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        '"': '&quot;',
        "'": '&#39;',
      }[s])
  );
}

const appTimeFormatter = new Intl.DateTimeFormat([], {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
});
const appDateFormatter = new Intl.DateTimeFormat([], {
  month: 'short',
  day: 'numeric',
});

function formatDate(str) {
  if (!str) return '—';
  try {
    const d = new Date(str.includes('Z') ? str : str + 'Z');
    return `${appTimeFormatter.format(d)} ${appDateFormatter.format(d)}`;
  } catch {
    return str;
  }
}

// showToast is provided globally by static/toast.js

// ── Status Badge Embed Modal Logic ───────────────────────────────────────────

let currentBadgeMonitor = { id: '', name: '' };
let currentBadgeStyle = 'shield';
let currentBadgeFormat = 'markdown';

function estimateBadgeTextWidth(text) {
  let width = 0;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (['m', 'M', 'W', 'w', '@'].includes(ch)) width += 9.2;
    else if (ch >= 'A' && ch <= 'Z') width += 7.5;
    else if (['i', 'l', '!', '|', '.', ':', ';', "'", ' '].includes(ch)) width += 3.6;
    else if (['f', 'r', 't', 'j'].includes(ch)) width += 4.8;
    else if ((ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9') || ch === '-' || ch === '_') width += 6.2;
    else width += 6.5;
  }
  return width;
}

function renderBadgePreviewSvg(name, status, style) {
  const normStatus = (status || 'UP').toUpperCase();
  let statusText = 'operational';
  let color = '#30D158';
  let textColor = '#09090b';

  if (normStatus === 'DOWN' || normStatus === 'DOWNTIME' || normStatus === 'OUTAGE') {
    statusText = 'downtime';
    color = '#FF453A';
    textColor = '#ffffff';
  } else if (normStatus === 'DEGRADED') {
    statusText = 'degraded';
    color = '#FFD60A';
    textColor = '#09090b';
  } else if (normStatus === 'MAINTENANCE') {
    statusText = 'maintenance';
    color = '#A855F7';
    textColor = '#ffffff';
  }

  const safeName = escapeHtml(name || 'Service');
  const safeStatus = escapeHtml(statusText);

  if (style === 'dot') {
    const statusUpper = safeStatus.toUpperCase();
    const fullText = `${name}: ${statusText.toUpperCase()}`;
    const width = Math.max(160, Math.round(fullText.length * 7.5) + 38);
    return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="28" viewBox="0 0 ${width} 28" fill="none" role="img" aria-label="${safeName}: ${statusUpper}">
  <title>${safeName}: ${statusUpper}</title>
  <rect width="${width}" height="28" rx="6" fill="#18181b" stroke="rgba(255,255,255,0.1)"/>
  <circle cx="16" cy="14" r="4" fill="${color}">
    <animate attributeName="opacity" values="0.7;1;0.7" dur="2s" repeatCount="indefinite"/>
  </circle>
  <circle cx="16" cy="14" r="7" stroke="${color}" stroke-width="1.2" fill="none" opacity="0.4">
    <animate attributeName="r" values="4;8" dur="2s" repeatCount="indefinite"/>
    <animate attributeName="opacity" values="0.6;0" dur="2s" repeatCount="indefinite"/>
  </circle>
  <text x="28" y="18" fill="#e4e4e7" font-family="monospace,Courier,monospace" font-size="11" font-weight="600">${safeName}: ${statusUpper}</text>
</svg>`;
  }

  if (style === 'compact') {
    const labelUpper = escapeHtml((name || 'Service').toUpperCase());
    const statusUpper = safeStatus.toUpperCase();
    const fullText = `${(name || 'Service').toUpperCase()} // ${statusText.toUpperCase()}`;
    const width = Math.max(170, Math.round(fullText.length * 7.2) + 32);
    const endX = width - 10;
    return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="26" viewBox="0 0 ${width} 26" fill="none" role="img" aria-label="${labelUpper} // ${statusUpper}">
  <title>${labelUpper} // ${statusUpper}</title>
  <rect width="${width}" height="26" rx="4" fill="#09090b" stroke="${color}" stroke-opacity="0.4" stroke-width="1"/>
  <rect x="5" y="5" width="5" height="16" rx="1.5" fill="${color}"/>
  <text x="16" y="17" fill="#a1a1aa" font-family="monospace,Courier,monospace" font-size="10" font-weight="600">${labelUpper}</text>
  <text x="${endX}" y="17" text-anchor="end" fill="${color}" font-family="monospace,Courier,monospace" font-size="10" font-weight="700">${statusUpper}</text>
</svg>`;
  }

  // Classic Shield (2-part vector badge matching KeepUp standard)
  const leftLabel = `KeepUp | ${name || 'Service'}`;
  const rightLabel = statusText;
  const leftEsc = escapeHtml(leftLabel);
  const rightEsc = escapeHtml(rightLabel);
  const leftW = Math.max(40, Math.round(estimateBadgeTextWidth(leftLabel) + 16));
  const rightW = Math.max(40, Math.round(estimateBadgeTextWidth(rightLabel) + 16));
  const totalW = leftW + rightW;
  const leftCenterX = Math.round(leftW / 2);
  const rightCenterX = Math.round(leftW + rightW / 2);
  const clipId = `k-badge-preview-${totalW}`;

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${totalW}" height="20" role="img" aria-label="${leftEsc}: ${rightEsc}">
  <title>${leftEsc}: ${rightEsc}</title>
  <clipPath id="${clipId}">
    <rect width="${totalW}" height="20" rx="4" fill="#fff"/>
  </clipPath>
  <g clip-path="url(#${clipId})">
    <rect width="${leftW}" height="20" fill="#18181b"/>
    <rect x="${leftW}" width="${rightW}" height="20" fill="${color}"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="system-ui,-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,sans-serif" text-rendering="geometricPrecision" font-size="11">
    <text x="${leftCenterX}" y="14" fill="#a1a1aa">${leftEsc}</text>
    <text x="${rightCenterX}" y="14" fill="${textColor}" font-weight="600">${rightEsc}</text>
  </g>
</svg>`;
}

function updateBadgeModalContent() {
  if (!currentBadgeMonitor.id) return;

  const origin = (window.location.origin && window.location.origin !== 'null')
    ? window.location.origin
    : 'http://localhost:8080';
  const badgeUrl = `${origin}/api/badge/${currentBadgeMonitor.id}?style=${currentBadgeStyle}`;

  let snippet = '';
  if (currentBadgeFormat === 'markdown') {
    snippet = `![${currentBadgeMonitor.name} Status](${badgeUrl})`;
  } else if (currentBadgeFormat === 'html') {
    snippet = `<img src="${badgeUrl}" alt="${escapeHtml(currentBadgeMonitor.name)} Status" />`;
  } else {
    snippet = badgeUrl;
  }

  const codeEl = document.getElementById('badge-modal-code');
  if (codeEl) {
    codeEl.textContent = snippet;
  }

  let liveStatus = 'UP';
  const live = statusMap[currentBadgeMonitor.id];
  if (live && live.current_status) {
    liveStatus = live.current_status;
  } else if (Array.isArray(window.allMonitors)) {
    const found = window.allMonitors.find((m) => m.id === currentBadgeMonitor.id);
    if (found && found.last_status) {
      liveStatus = found.last_status;
    }
  }

  const previewContainer = document.getElementById('badge-modal-preview');
  if (previewContainer) {
    previewContainer.innerHTML = renderBadgePreviewSvg(
      currentBadgeMonitor.name,
      liveStatus,
      currentBadgeStyle
    );
  }
}

function setBadgeModalStyle(style) {
  currentBadgeStyle = style;

  const styles = ['shield', 'dot', 'compact'];
  styles.forEach((s) => {
    const btn = document.getElementById(`badge-style-${s}`);
    if (!btn) return;
    if (s === style) {
      btn.className =
        'badge-style-btn py-2 px-2.5 rounded-xl border text-xs font-medium text-center transition flex flex-col items-center gap-0.5 bg-black/5 dark:bg-white/10 border-emerald-500/50 text-neutral-900 dark:text-white shadow-sm font-semibold';
    } else {
      btn.className =
        'badge-style-btn py-2 px-2.5 rounded-xl border text-xs font-medium text-center transition flex flex-col items-center gap-0.5 border-black/5 dark:border-white/10 text-neutral-500 dark:text-white/60 hover:text-neutral-900 dark:hover:text-white hover:bg-black/5 dark:hover:bg-white/5';
    }
  });

  updateBadgeModalContent();
}

function setBadgeModalFormat(format) {
  currentBadgeFormat = format;

  const formats = ['markdown', 'html', 'url'];
  formats.forEach((f) => {
    const btn = document.getElementById(`badge-fmt-${f}`);
    if (!btn) return;
    if (f === format) {
      btn.className =
        'badge-fmt-btn py-1 px-3 rounded-lg transition-all text-xs font-mono bg-white dark:bg-white/10 text-neutral-900 dark:text-white shadow-sm font-semibold';
    } else {
      btn.className =
        'badge-fmt-btn py-1 px-3 rounded-lg transition-all text-xs font-mono text-neutral-500 dark:text-white/50 hover:text-neutral-900 dark:hover:text-white';
    }
  });

  updateBadgeModalContent();
}

function openBadgeModal(monitorId, monitorName) {
  currentBadgeMonitor = { id: monitorId, name: monitorName };

  const titleEl = document.getElementById('badge-modal-title');
  const subtitleEl = document.getElementById('badge-modal-subtitle');
  if (titleEl) titleEl.innerText = monitorName;
  if (subtitleEl) subtitleEl.innerText = `Status Badge Embed • /api/badge/${monitorId}`;

  // Apply current active styles and format
  setBadgeModalStyle(currentBadgeStyle || 'shield');
  setBadgeModalFormat(currentBadgeFormat || 'markdown');

  const backdrop = document.getElementById('badge-modal');
  const card = document.getElementById('badge-modal-card');
  if (!backdrop || !card) return;

  backdrop.classList.remove('hidden');
  requestAnimationFrame(() => {
    card.classList.remove('scale-95', 'opacity-0');
    card.classList.add('scale-100', 'opacity-100');
  });
}

function closeBadgeModal() {
  const backdrop = document.getElementById('badge-modal');
  const card = document.getElementById('badge-modal-card');
  if (!backdrop || !card) return;

  card.classList.remove('scale-100', 'opacity-100');
  card.classList.add('scale-95', 'opacity-0');

  setTimeout(() => {
    backdrop.classList.add('hidden');
  }, 200);
}

async function copyBadgeModalCode(btn) {
  const codeEl = document.getElementById('badge-modal-code');
  if (!codeEl) return;
  const text = codeEl.innerText || codeEl.textContent;
  if (!text) return;

  const btnEl = btn || document.getElementById('badge-modal-copy-btn');
  const label = btnEl ? (btnEl.querySelector('.copy-label') || btnEl.querySelector('span') || btnEl) : null;
  const originalText = label ? label.innerText : 'Copy Code';

  const onSuccess = () => {
    if (label) {
      label.innerText = 'Copied!';
      setTimeout(() => {
        label.innerText = originalText;
      }, 2000);
    }
    if (typeof showToast === 'function') {
      showToast({
        title: 'Badge Copied',
        message: 'Embed snippet copied to clipboard',
        type: 'success',
      });
    }
  };

  try {
    await navigator.clipboard.writeText(text);
    onSuccess();
  } catch (_) {
    const textarea = document.createElement('textarea');
    textarea.value = text;
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    document.body.appendChild(textarea);
    textarea.select();
    try {
      document.execCommand('copy');
      onSuccess();
    } catch (e) {
      if (typeof showToast === 'function') {
        showToast({
          title: 'Copy Failed',
          message: 'Could not copy to clipboard',
          type: 'error',
        });
      }
    }
    document.body.removeChild(textarea);
  }
}

// Global exports for backward compatibility
window.openBadgeModal = openBadgeModal;
window.closeBadgeModal = closeBadgeModal;
window.setBadgeModalStyle = setBadgeModalStyle;
window.setBadgeModalFormat = setBadgeModalFormat;
window.copyBadgeModalCode = copyBadgeModalCode;
window.openPasswordModal = openPasswordModal;
window.closePasswordModal = closePasswordModal;
window.handlePasswordSubmit = handlePasswordSubmit;
window.openMonitorDetailModal = openMonitorDetailModal;
window.closeMonitorDetailModal = closeMonitorDetailModal;
window.triggerDetailPing = triggerDetailPing;
window.toggleMonitorActive = toggleMonitorActive;
window.toggleWebhookActive = toggleWebhookActive;
window.testWebhookDirect = testWebhookDirect;

// ── Admin Password Update Modal Controller ───────────────────────────────────

function openPasswordModal() {
  const modal = document.getElementById('password-modal');
  const errorContainer = document.getElementById('password-modal-error');
  const form = document.getElementById('password-form');
  if (errorContainer) {
    errorContainer.classList.add('hidden');
    errorContainer.textContent = '';
    errorContainer.className = 'hidden text-xs text-rose-400 bg-rose-500/10 border border-rose-500/20 p-2.5 rounded-lg font-mono';
  }
  if (form) form.reset();
  if (modal) {
    modal.classList.remove('hidden');
    const currInput = document.getElementById('current-password');
    if (currInput) setTimeout(() => currInput.focus(), 50);
  }
}

function closePasswordModal() {
  const modal = document.getElementById('password-modal');
  if (modal) {
    modal.classList.add('hidden');
  }
  const form = document.getElementById('password-form');
  if (form) form.reset();
  const errorContainer = document.getElementById('password-modal-error');
  if (errorContainer) {
    errorContainer.classList.add('hidden');
    errorContainer.textContent = '';
    errorContainer.className = 'hidden text-xs text-rose-400 bg-rose-500/10 border border-rose-500/20 p-2.5 rounded-lg font-mono';
  }
}

async function handlePasswordSubmit(e) {
  if (e && e.preventDefault) e.preventDefault();

  const currentPassword = document.getElementById('current-password')?.value || '';
  const newPassword = document.getElementById('new-password')?.value || '';
  const confirmPassword = document.getElementById('confirm-password')?.value || '';
  const errorContainer = document.getElementById('password-modal-error');
  const submitBtn = document.getElementById('password-submit-btn');

  function showError(msg) {
    if (errorContainer) {
      errorContainer.textContent = msg;
      errorContainer.className = 'text-xs text-rose-400 bg-rose-500/10 border border-rose-500/20 p-2.5 rounded-lg font-mono';
      errorContainer.classList.remove('hidden');
    }
  }

  function showSuccess(msg) {
    if (errorContainer) {
      errorContainer.textContent = msg;
      errorContainer.className = 'text-xs text-emerald-400 bg-emerald-500/10 border border-emerald-500/20 p-2.5 rounded-lg font-mono';
      errorContainer.classList.remove('hidden');
    }
  }

  if (newPassword.length < 8) {
    showError('New password must be at least 8 characters long');
    return;
  }

  if (newPassword !== confirmPassword) {
    showError('New passwords do not match');
    return;
  }

  if (submitBtn) {
    submitBtn.disabled = true;
    submitBtn.textContent = 'Updating...';
  }

  try {
    const res = await apiFetch('/api/admin/change-password', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        current_password: currentPassword,
        new_password: newPassword,
      }),
      skipAuthRedirect: true,
    });

    const data = await res.json().catch(() => ({}));

    if (!res.ok) {
      const errMsg = data.error || 'Failed to update password';
      showError(errMsg);
      return;
    }

    // Success state
    showSuccess(data.message || 'Password updated successfully');
    const form = document.getElementById('password-form');
    if (form) form.reset();

    if (typeof showToast === 'function') {
      showToast({
        title: 'Security',
        message: 'Admin password updated successfully',
        type: 'success',
      });
    }

    setTimeout(() => {
      closePasswordModal();
    }, 1500);
  } catch (err) {
    showError('Network error while updating password');
  } finally {
    if (submitBtn) {
      submitBtn.disabled = false;
      submitBtn.textContent = 'Save New Password';
    }
  }
}

// ── Monitor Detail View Controller ──────────────────────────────────────────

let currentDetailMonitorId = null;

async function openMonitorDetailModal(id) {
  currentDetailMonitorId = id;
  const modal = document.getElementById('monitor-detail-modal');
  if (!modal) return;

  modal.classList.remove('hidden');

  // Loading skeleton state
  const nameEl = document.getElementById('detail-modal-name');
  const urlEl = document.getElementById('detail-modal-url');
  const tbodyEl = document.getElementById('detail-logs-tbody');
  const chartEl = document.getElementById('detail-latency-chart');
  const countEl = document.getElementById('detail-logs-count');

  if (nameEl) nameEl.textContent = 'Loading monitor telemetry...';
  if (urlEl) urlEl.textContent = '—';
  if (countEl) countEl.textContent = '...';
  if (chartEl) {
    chartEl.innerHTML = '<div class="text-xs text-neutral-500 font-mono py-6">Loading latency telemetry...</div>';
  }
  if (tbodyEl) {
    tbodyEl.innerHTML = '<tr><td colspan="4" class="py-6 text-center text-neutral-500 font-sans">Loading check history...</td></tr>';
  }

  try {
    const fetchFn = window.apiFetch || window.fetch;
    const res = await fetchFn(`/api/admin/monitors/${encodeURIComponent(id)}/details`, {
      credentials: 'include',
    });

    if (!res.ok) {
      if (nameEl) nameEl.textContent = 'Monitor Details';
      if (tbodyEl) {
        tbodyEl.innerHTML = `<tr><td colspan="4" class="py-6 text-center text-rose-400 font-sans">Failed to load telemetry (${res.status} ${res.statusText})</td></tr>`;
      }
      return;
    }

    const data = await res.json();
    renderMonitorDetailData(data);
  } catch (err) {
    if (nameEl) nameEl.textContent = 'Monitor Details';
    if (tbodyEl) {
      tbodyEl.innerHTML = '<tr><td colspan="4" class="py-6 text-center text-rose-400 font-sans">Unable to connect to server</td></tr>';
    }
  }
}

function closeMonitorDetailModal() {
  const modal = document.getElementById('monitor-detail-modal');
  if (modal) {
    modal.classList.add('hidden');
  }
  currentDetailMonitorId = null;
}

function renderDetailHourlySvg(hourlyList) {
  const list = Array.isArray(hourlyList) && hourlyList.length > 0 ? hourlyList : [];
  const items = [];
  for (let i = 0; i < 24; i++) {
    items.push(list[i] || { avg_latency_ms: 0, status: 'NO_DATA', timestamp: '' });
  }

  const maxLat = Math.max(
    ...items.map((h) => (typeof h.avg_latency_ms === 'number' ? h.avg_latency_ms : 0)),
    50
  );

  const bars = items
    .map((item, idx) => {
      const lat = typeof item.avg_latency_ms === 'number' ? item.avg_latency_ms : 0;
      const status = item.status || 'NO_DATA';
      let fill = 'rgba(255, 255, 255, 0.12)';
      if (status === 'UP') fill = '#30D158';
      else if (status === 'DEGRADED') fill = '#FFD60A';
      else if (status === 'DOWN') fill = '#FF453A';

      const barHeight = Math.max(4, Math.round((lat / maxLat) * 55));
      const x = idx * 20 + 3;
      const y = 65 - barHeight;

      let timeLabel = `${24 - idx}h ago`;
      if (item.timestamp) {
        try {
          timeLabel = new Date(item.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
        } catch (_) {}
      }

      return `<rect x="${x}" y="${y}" width="14" height="${barHeight}" rx="2" fill="${fill}" class="transition-opacity hover:opacity-75 cursor-pointer">
        <title>${timeLabel}: ${lat} ms (${status})</title>
      </rect>`;
    })
    .join('');

  return `<svg viewBox="0 0 480 70" class="w-full h-16 overflow-visible" preserveAspectRatio="none">${bars}</svg>`;
}

function renderMonitorDetailData(data) {
  if (!data || !data.monitor) return;
  const m = data.monitor;
  const slas = data.slas || { uptime_24h: 100, uptime_7d: 100, uptime_30d: 100 };
  const lat = data.latency || { p50: 0, p95: 0, p99: 0, avg: 0 };
  const logs = data.logs || [];
  const hourly = data.hourly_latency || [];

  // 1. Header Section
  const nameEl = document.getElementById('detail-modal-name');
  const urlEl = document.getElementById('detail-modal-url');
  const methodEl = document.getElementById('detail-modal-method');
  const statusBadge = document.getElementById('detail-modal-status-badge');

  if (nameEl) nameEl.textContent = m.name || 'Monitor';
  if (urlEl) urlEl.textContent = m.url || '';
  if (methodEl) methodEl.textContent = m.method || 'GET';

  if (statusBadge) {
    const isPaused = !m.is_active;
    const status = m.status || (isPaused ? 'PAUSED' : 'UP');
    statusBadge.textContent = status;

    if (isPaused) {
      statusBadge.className = 'px-2.5 py-0.5 rounded-full text-[10px] font-medium border uppercase tracking-wider bg-neutral-500/15 text-neutral-400 border-neutral-500/20';
    } else if (status === 'MAINTENANCE') {
      statusBadge.className = 'px-2.5 py-0.5 rounded-full text-[10px] font-medium border uppercase tracking-wider bg-purple-500/15 text-purple-400 border-purple-500/20';
    } else if (status === 'DOWN') {
      statusBadge.className = 'px-2.5 py-0.5 rounded-full text-[10px] font-medium border uppercase tracking-wider bg-rose-500/15 text-rose-400 border-rose-500/20';
    } else if (status === 'DEGRADED') {
      statusBadge.className = 'px-2.5 py-0.5 rounded-full text-[10px] font-medium border uppercase tracking-wider bg-amber-500/15 text-amber-400 border-amber-500/20';
    } else {
      statusBadge.className = 'px-2.5 py-0.5 rounded-full text-[10px] font-medium border uppercase tracking-wider bg-emerald-500/15 text-emerald-400 border-emerald-500/20';
    }
  }

  const domainBadge = document.getElementById('detail-modal-domain-badge');
  if (domainBadge) {
    if (m.check_domain_expiry && m.domain_days_remaining !== null && m.domain_days_remaining !== undefined) {
      const days = m.domain_days_remaining;
      const warningDays = m.domain_warning_days || 30;
      domainBadge.classList.remove('hidden');
      if (days <= 0) {
        domainBadge.className = 'px-2.5 py-0.5 rounded-full text-[10px] font-mono font-medium border bg-rose-500/15 text-rose-400 border-rose-500/20';
        domainBadge.textContent = '🔴 Domain Expired';
      } else if (days <= warningDays) {
        domainBadge.className = 'px-2.5 py-0.5 rounded-full text-[10px] font-mono font-medium border bg-amber-500/15 text-amber-400 border-amber-500/20';
        domainBadge.textContent = `⚠️ ${days}d domain left`;
      } else {
        domainBadge.className = 'px-2.5 py-0.5 rounded-full text-[10px] font-mono font-medium border bg-white/5 text-neutral-300 border-white/10';
        domainBadge.textContent = `🌐 ${days}d domain left`;
      }
    } else {
      domainBadge.classList.add('hidden');
    }
  }

  const sslBadge = document.getElementById('detail-modal-ssl-badge');
  if (sslBadge) {
    if (m.check_ssl && m.ssl_days_remaining !== null && m.ssl_days_remaining !== undefined) {
      const days = m.ssl_days_remaining;
      const warningDays = m.ssl_warning_days || 30;
      sslBadge.classList.remove('hidden');
      if (days <= 0) {
        sslBadge.className = 'px-2.5 py-0.5 rounded-full text-[10px] font-mono font-medium border bg-rose-500/15 text-rose-400 border-rose-500/20';
        sslBadge.textContent = '🔴 SSL Expired';
      } else if (days <= warningDays) {
        sslBadge.className = 'px-2.5 py-0.5 rounded-full text-[10px] font-mono font-medium border bg-amber-500/15 text-amber-400 border-amber-500/20';
        sslBadge.textContent = `🔒 ${days}d SSL left`;
      } else {
        sslBadge.className = 'px-2.5 py-0.5 rounded-full text-[10px] font-mono font-medium border bg-white/5 text-neutral-300 border-white/10';
        sslBadge.textContent = `🔒 ${days}d SSL left`;
      }
    } else {
      sslBadge.classList.add('hidden');
    }
  }

  // 2. SLA Telemetry Row (3 Cards)
  const sla24 = document.getElementById('detail-sla-24h');
  const sla7d = document.getElementById('detail-sla-7d');
  const sla30 = document.getElementById('detail-sla-30d');

  if (sla24) {
    sla24.textContent = `${slas.uptime_24h.toFixed(2)}%`;
    sla24.className = `text-lg sm:text-xl font-bold font-mono mt-1 ${slas.uptime_24h >= 99 ? 'text-emerald-400' : slas.uptime_24h >= 90 ? 'text-amber-400' : 'text-rose-400'}`;
  }
  if (sla7d) {
    sla7d.textContent = `${slas.uptime_7d.toFixed(2)}%`;
    sla7d.className = `text-lg sm:text-xl font-bold font-mono mt-1 ${slas.uptime_7d >= 99 ? 'text-emerald-400' : slas.uptime_7d >= 90 ? 'text-amber-400' : 'text-rose-400'}`;
  }
  if (sla30) {
    sla30.textContent = `${slas.uptime_30d.toFixed(2)}%`;
    sla30.className = `text-lg sm:text-xl font-bold font-mono mt-1 ${slas.uptime_30d >= 99 ? 'text-emerald-400' : slas.uptime_30d >= 90 ? 'text-amber-400' : 'text-rose-400'}`;
  }

  // 3. Latency Breakdown Bar
  const p50El = document.getElementById('detail-lat-p50');
  const p95El = document.getElementById('detail-lat-p95');
  const p99El = document.getElementById('detail-lat-p99');
  const avgEl = document.getElementById('detail-lat-avg');

  if (p50El) p50El.textContent = `${lat.p50}ms`;
  if (p95El) p95El.textContent = `${lat.p95}ms`;
  if (p99El) p99El.textContent = `${lat.p99}ms`;
  if (avgEl) avgEl.textContent = `${lat.avg}ms`;

  // 4. Interactive SVG Latency Chart
  const chartEl = document.getElementById('detail-latency-chart');
  if (chartEl) {
    chartEl.innerHTML = renderDetailHourlySvg(hourly);
  }

  // 5. Recent Check Logs Table
  const countEl = document.getElementById('detail-logs-count');
  const tbodyEl = document.getElementById('detail-logs-tbody');

  if (countEl) countEl.textContent = `${logs.length} logs`;
  if (tbodyEl) {
    if (logs.length === 0) {
      tbodyEl.innerHTML = '<tr><td colspan="4" class="py-6 text-center text-neutral-500 font-sans">No check logs recorded yet for this target.</td></tr>';
      return;
    }

    const rowsHtml = logs.map((log) => {
      const isUp = log.is_up;
      const statusNum = log.status_code;
      const is2xx = statusNum && statusNum >= 200 && statusNum < 300;

      const rowClass = isUp && is2xx
        ? 'hover:bg-white/5 transition'
        : 'bg-rose-500/10 hover:bg-rose-500/15 text-rose-200 transition';

      const statusTag = is2xx
        ? `<span class="text-emerald-400 font-semibold">${statusNum}</span>`
        : `<span class="text-rose-400 font-semibold">${statusNum || '—'}</span>`;

      const resultTag = isUp
        ? '<span class="text-emerald-400">UP</span>'
        : `<span class="text-rose-400 font-semibold">${log.error_message ? escapeHtml(log.error_message) : 'Check Failed'}</span>`;

      let dateStr = log.created_at;
      try {
        dateStr = new Date(log.created_at).toLocaleString();
      } catch (_) {}

      return `<tr class="${rowClass}">
        <td class="py-2 px-3 text-neutral-400">${dateStr}</td>
        <td class="py-2 px-3">${statusTag}</td>
        <td class="py-2 px-3 text-white">${log.response_time_ms !== null ? log.response_time_ms + ' ms' : '—'}</td>
        <td class="py-2 px-3 max-w-xs truncate">${resultTag}</td>
      </tr>`;
    }).join('');

    tbodyEl.innerHTML = rowsHtml;
  }
}

async function triggerDetailPing() {
  if (!currentDetailMonitorId) return;

  const btn = document.getElementById('detail-modal-ping-btn');
  const icon = document.getElementById('detail-ping-icon');
  const text = document.getElementById('detail-ping-text');

  if (btn) btn.disabled = true;
  if (icon) icon.textContent = '⏳';
  if (text) text.textContent = 'Pinging...';

  try {
    const fetchFn = window.apiFetch || window.fetch;
    const res = await fetchFn(`/api/admin/monitors/${encodeURIComponent(currentDetailMonitorId)}/ping`, {
      method: 'POST',
      credentials: 'include',
      skipAuthRedirect: true,
    });

    if (res.status === 401) {
      if (typeof showToast === 'function') {
        showToast({
          title: 'Authentication Required',
          message: 'Admin login required to trigger on-demand ping.',
          type: 'warning',
        });
      }
      return;
    }

    if (!res.ok) {
      if (typeof showToast === 'function') {
        showToast({
          title: 'Ping Failed',
          message: `Server returned HTTP ${res.status}`,
          type: 'error',
        });
      }
      return;
    }

    const pingData = await res.json();
    if (typeof showToast === 'function') {
      showToast({
        title: `Ping Result: ${pingData.status}`,
        message: `HTTP ${pingData.status_code || '—'} in ${pingData.latency_ms} ms`,
        type: pingData.status === 'UP' ? 'success' : 'error',
      });
    }

    // Refresh telemetry immediately
    const detailsRes = await fetchFn(`/api/admin/monitors/${encodeURIComponent(currentDetailMonitorId)}/details`, {
      credentials: 'include',
    });
    if (detailsRes.ok) {
      const detailsData = await detailsRes.json();
      renderMonitorDetailData(detailsData);
    }

    // Trigger dashboard refresh if available
    if (typeof fetchDashboardData === 'function') {
      fetchDashboardData(false);
    }
  } catch (err) {
    if (typeof showToast === 'function') {
      showToast({
        title: 'Ping Error',
        message: err.message || 'Network error executing ping',
        type: 'error',
      });
    }
  } finally {
    if (btn) btn.disabled = false;
    if (icon) icon.textContent = '⚡';
    if (text) text.textContent = 'Ping Now';
  }
}

// ── Centralized Event Delegation ─────────────────────────────────────────────

document.addEventListener('click', (e) => {
  const detailCard = e.target.closest('[data-action="open-detail"]');
  if (detailCard && !e.target.closest('button, a, input, select, textarea, label')) {
    const id = detailCard.dataset.id;
    if (id) openMonitorDetailModal(id);
    return;
  }

  const target = e.target.closest('[data-action]');
  if (!target) return;

  const action = target.dataset.action;
  switch (action) {
    case 'open-detail': {
      const id = target.dataset.id;
      if (id) openMonitorDetailModal(id);
      break;
    }
    case 'close-detail-modal': {
      closeMonitorDetailModal();
      break;
    }
    case 'toggle-active': {
      const id = target.dataset.id;
      const isActive = target.dataset.active === 'true';
      toggleMonitorActive(id, isActive);
      break;
    }
    case 'open-badge': {
      const id = target.dataset.id;
      const name = target.dataset.name;
      openBadgeModal(id, name);
      break;
    }
    case 'edit-monitor': {
      const id = target.dataset.id;
      if (typeof window.openEditMonitorModal === 'function') {
        window.openEditMonitorModal(id);
      }
      break;
    }
    case 'view-logs': {
      const id = target.dataset.id;
      const name = target.dataset.name;
      const url = target.dataset.url;
      if (typeof window.viewLogs === 'function') {
        window.viewLogs(id, name, url);
      }
      break;
    }
    case 'delete-monitor': {
      const id = target.dataset.id;
      const name = target.dataset.name;
      if (typeof window.deleteMonitor === 'function') {
        window.deleteMonitor(id, name);
      }
      break;
    }
    case 'test-webhook': {
      const id = target.dataset.id;
      const name = target.dataset.name;
      testWebhookDirect(id, name);
      break;
    }
    case 'edit-channel': {
      const id = target.dataset.id;
      if (id) openEditChannelModal(id);
      break;
    }
    case 'delete-channel': {
      const id = target.dataset.id;
      const name = target.dataset.name;
      if (id) deleteNotificationChannel(id, name);
      break;
    }
    case 'test-channel': {
      const id = target.dataset.id;
      const name = target.dataset.name;
      if (id) testNotificationChannel(id, name, target);
      break;
    }
    case 'open-update-incident': {
      const id = target.dataset.id;
      if (id) openAddIncidentUpdateModal(id);
      break;
    }
    case 'resolve-incident': {
      const id = target.dataset.id;
      if (id) resolveIncident(id);
      break;
    }
    case 'delete-incident': {
      const id = target.dataset.id;
      if (id) deleteIncident(id);
      break;
    }
    case 'refresh-asset': {
      const id = target.dataset.id;
      if (id) forceRefreshAssetRow(id, target);
      break;
    }
    case 'open-create-channel': {
      openCreateChannelModal();
      break;
    }
    case 'close-badge-modal': {
      closeBadgeModal();
      break;
    }
    case 'open-password-modal': {
      openPasswordModal();
      break;
    }
    case 'close-password-modal': {
      closePasswordModal();
      break;
    }
    case 'badge-style': {
      const style = target.dataset.style;
      if (style) setBadgeModalStyle(style);
      break;
    }
    case 'badge-fmt': {
      const fmt = target.dataset.fmt;
      if (fmt) setBadgeModalFormat(fmt);
      break;
    }
    case 'badge-copy': {
      copyBadgeModalCode(target);
      break;
    }
    default:
      break;
  }
});

document.addEventListener('change', (e) => {
  const target = e.target.closest('[data-action="toggle-webhook"]');
  if (target) {
    const id = target.dataset.id;
    const currentEnabled = target.dataset.enabled === 'true';
    toggleWebhookActive(id, currentEnabled);
  }
});

// Escape key listener for modals
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    const badgeModal = document.getElementById('badge-modal');
    if (badgeModal && !badgeModal.classList.contains('hidden')) {
      closeBadgeModal();
    }
    const passwordModal = document.getElementById('password-modal');
    if (passwordModal && !passwordModal.classList.contains('hidden')) {
      closePasswordModal();
    }
    const detailModal = document.getElementById('monitor-detail-modal');
    if (detailModal && !detailModal.classList.contains('hidden')) {
      closeMonitorDetailModal();
    }
  }
});

// ── Asset Expiry Navigation & Table Interactivity ─────────────────────────────

let currentAdminView = 'monitors';
let isAssetExpiryLoading = false;
let isChannelsLoading = false;
let isChannelModalInline = false;
window.cachedChannels = [];

function switchAdminView(view) {
  if (view === 'status-pages') {
    openStatusPagesModal();
    return;
  }
  if (view === 'maintenance') {
    openMaintenanceModal();
    return;
  }

  currentAdminView = view;
  const monitorsPanel = document.getElementById('monitors-panel');
  const assetExpiryPanel = document.getElementById('asset-expiry-panel');
  const channelsPanel = document.getElementById('channels-panel');
  const incidentsPanel = document.getElementById('incidents-panel');
  const navMonitorsBtn = document.getElementById('nav-btn-monitors');
  const navStatusPagesBtn = document.getElementById('nav-btn-status-pages');
  const navAssetsBtn = document.getElementById('nav-btn-assets');
  const navChannelsBtn = document.getElementById('nav-btn-channels');
  const navMaintenanceBtn = document.getElementById('nav-btn-maintenance');
  const navIncidentsBtn = document.getElementById('nav-btn-incidents');

  if (monitorsPanel) monitorsPanel.classList.add('hidden');
  if (assetExpiryPanel) assetExpiryPanel.classList.add('hidden');
  if (channelsPanel) channelsPanel.classList.add('hidden');
  if (incidentsPanel) incidentsPanel.classList.add('hidden');

  const inactiveBtnClass = 'h-8 px-3 rounded-lg transition text-white/60 hover:text-white hover:bg-white/10 border border-transparent flex items-center gap-1.5 whitespace-nowrap cursor-pointer';
  const activeBtnClass = 'h-8 px-3 rounded-lg transition bg-white/15 text-white border border-white/20 shadow-sm font-medium flex items-center gap-1.5 whitespace-nowrap cursor-pointer';

  if (navMonitorsBtn) navMonitorsBtn.className = inactiveBtnClass;
  if (navStatusPagesBtn) navStatusPagesBtn.className = inactiveBtnClass;
  if (navAssetsBtn) navAssetsBtn.className = inactiveBtnClass;
  if (navChannelsBtn) navChannelsBtn.className = inactiveBtnClass;
  if (navMaintenanceBtn) navMaintenanceBtn.className = inactiveBtnClass;
  if (navIncidentsBtn) navIncidentsBtn.className = inactiveBtnClass;

  if (view === 'assets') {
    if (assetExpiryPanel) assetExpiryPanel.classList.remove('hidden');
    if (navAssetsBtn) navAssetsBtn.className = activeBtnClass;
    loadAssetExpiryData(true);
  } else if (view === 'channels') {
    if (channelsPanel) channelsPanel.classList.remove('hidden');
    if (navChannelsBtn) navChannelsBtn.className = activeBtnClass;
    loadNotificationChannels(true);
  } else if (view === 'incidents') {
    if (incidentsPanel) incidentsPanel.classList.remove('hidden');
    if (navIncidentsBtn) navIncidentsBtn.className = activeBtnClass;
    loadAdminIncidents(true);
  } else {
    if (monitorsPanel) monitorsPanel.classList.remove('hidden');
    if (navMonitorsBtn) navMonitorsBtn.className = activeBtnClass;
    fetchDashboardData(true);
  }
}
window.switchAdminView = switchAdminView;

async function loadAssetExpiryData(showLoader = true) {
  if (isAssetExpiryLoading) return;
  isAssetExpiryLoading = true;

  const refreshIcon = document.getElementById('asset-refresh-icon');
  if (refreshIcon && showLoader) {
    refreshIcon.classList.add('animate-spin');
  }

  try {
    const res = await apiFetch('/api/admin/assets/expiry', {
      method: 'GET',
      headers: { 'Content-Type': 'application/json' },
    });

    if (!res || !res.ok) {
      return;
    }

    const items = await res.json();
    renderAssetExpiryTable(items);
  } catch (err) {
    console.error('Failed to load asset expiry data:', err);
  } finally {
    isAssetExpiryLoading = false;
    if (refreshIcon) {
      refreshIcon.classList.remove('animate-spin');
    }
  }
}
window.loadAssetExpiryData = loadAssetExpiryData;

function renderAssetExpiryCell(enabled, daysRemaining, expiresAt, type) {
  if (!enabled) {
    return `<span class="text-neutral-500 font-mono text-xs">—</span>`;
  }
  if (daysRemaining === null || daysRemaining === undefined) {
    return `<span class="text-neutral-400 font-mono text-[11px] bg-white/5 px-2 py-0.5 rounded border border-white/5">Pending check</span>`;
  }

  const icon = type === 'ssl' ? '🔒' : '🌐';
  const label = daysRemaining <= 0 ? 'Expired' : `${daysRemaining}d left`;
  const titleAttr = expiresAt ? `title="Expires ${escapeHtml(expiresAt)}"` : '';

  if (daysRemaining <= 7) {
    return `<span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[11px] font-mono font-medium bg-rose-500/10 text-rose-400 border border-rose-500/20" ${titleAttr}>${icon} ${label}</span>`;
  } else if (daysRemaining <= 30) {
    return `<span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[11px] font-mono font-medium bg-amber-500/10 text-amber-400 border border-amber-500/20" ${titleAttr}>${icon} ${label}</span>`;
  } else {
    return `<span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[11px] font-mono text-neutral-400 bg-white/5 border border-white/5" ${titleAttr}>${icon} ${label}</span>`;
  }
}

function renderUrgencyBadge(urgency) {
  if (urgency === 'CRITICAL') {
    return `<span class="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-[10px] font-semibold bg-rose-500/10 text-rose-400 border border-rose-500/20"><span class="w-1.5 h-1.5 rounded-full bg-rose-400"></span>CRITICAL</span>`;
  } else if (urgency === 'WARNING') {
    return `<span class="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-[10px] font-semibold bg-amber-500/10 text-amber-400 border border-amber-500/20"><span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span>WARNING</span>`;
  } else {
    return `<span class="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-[10px] font-semibold bg-emerald-500/10 text-emerald-400 border border-emerald-500/20"><span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>HEALTHY</span>`;
  }
}

function renderAssetExpiryTable(items) {
  const critEl = document.getElementById('asset-stat-critical');
  const warnEl = document.getElementById('asset-stat-warning');
  const totalEl = document.getElementById('asset-stat-total');
  const tbody = document.getElementById('asset-expiry-tbody');

  const critCount = items.filter((i) => i.urgency === 'CRITICAL').length;
  const warnCount = items.filter((i) => i.urgency === 'WARNING').length;
  const totalCount = items.length;

  if (critEl) critEl.textContent = critCount;
  if (warnEl) warnEl.textContent = warnCount;
  if (totalEl) totalEl.textContent = totalCount;

  if (!tbody) return;

  if (items.length === 0) {
    tbody.innerHTML = `
      <tr>
        <td colspan="5" class="py-12 text-center text-white/40 font-sans text-xs">
          No assets currently tracked for SSL or Domain Expiration.<br>
          <span class="text-white/30 text-[11px] mt-1 inline-block">Enable "TLS / SSL Expiration Check" or "Domain Expiration Check" in monitor settings.</span>
        </td>
      </tr>
    `;
    return;
  }

  const rowsHtml = items.map((item) => {
    const sslCell = renderAssetExpiryCell(item.ssl_enabled, item.ssl_days_remaining, item.ssl_expires_at, 'ssl');
    const domainCell = renderAssetExpiryCell(item.domain_enabled, item.domain_days_remaining, item.domain_expires_at, 'domain');
    const urgencyCell = renderUrgencyBadge(item.urgency);

    return `
      <tr id="asset-row-${escapeHtml(item.monitor_id)}" class="hover:bg-white/5 transition">
        <td class="py-3 px-4">
          <div class="font-sans font-medium text-white text-sm">${escapeHtml(item.name)}</div>
          <div class="text-[11px] text-white/50 truncate max-w-xs font-mono">${escapeHtml(item.url)}</div>
        </td>
        <td class="py-3 px-4">${sslCell}</td>
        <td class="py-3 px-4">${domainCell}</td>
        <td class="py-3 px-4">${urgencyCell}</td>
        <td class="py-3 px-4 text-right">
          <button type="button" data-action="refresh-asset" data-id="${escapeHtml(item.monitor_id)}" class="px-2.5 py-1 text-xs rounded-lg bg-white/5 hover:bg-white/10 text-white/80 hover:text-white border border-white/10 transition inline-flex items-center gap-1 font-sans cursor-pointer active:scale-95" title="Force immediate TLS and domain check">
            <span>⚡</span><span>Force Refresh</span>
          </button>
        </td>
      </tr>
    `;
  }).join('');

  tbody.innerHTML = rowsHtml;
}

async function forceRefreshAssetRow(monitorId, btn) {
  if (btn) {
    btn.disabled = true;
    btn.innerHTML = `<svg class="w-3.5 h-3.5 animate-spin inline mr-1" fill="none" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path></svg> Refreshing...`;
  }

  try {
    const res = await apiFetch(`/api/admin/monitors/${monitorId}/refresh-assets`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
    });

    if (res && res.ok) {
      const updatedItem = await res.json();
      if (typeof showToast === 'function') {
        showToast({
          title: 'Asset Refreshed',
          message: `Live telemetry updated for "${updatedItem.name}"`,
          type: 'success',
        });
      }
      await loadAssetExpiryData(false);
    } else {
      const err = await res.json().catch(() => ({}));
      if (typeof showToast === 'function') {
        showToast({
          title: 'Refresh Failed',
          message: err.error || 'Unable to refresh asset telemetry',
          type: 'error',
        });
      }
    }
  } catch (err) {
    console.error('Error refreshing asset:', err);
    if (typeof showToast === 'function') {
      showToast({
        title: 'Refresh Error',
        message: err.message || 'Network error occurred',
        type: 'error',
      });
    }
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.innerHTML = `<span>⚡</span><span>Force Refresh</span>`;
    }
  }
}
window.forceRefreshAssetRow = forceRefreshAssetRow;

// ── Notification Channels Management & View ─────────────────────────────────

function maskUrl(url) {
  if (!url) return '';
  try {
    const parsed = new URL(url);
    const parts = parsed.pathname.split('/');
    if (parts.length > 2) {
      const last = parts[parts.length - 1];
      const masked = last.length > 8 ? last.slice(0, 4) + '••••' + last.slice(-4) : '••••••••';
      parts[parts.length - 1] = masked;
      return `${parsed.protocol}//${parsed.host}${parts.join('/')}`;
    }
    return `${parsed.protocol}//${parsed.host}${parsed.pathname.slice(0, 8)}••••`;
  } catch (_) {
    if (url.length > 24) {
      return url.slice(0, 16) + '••••' + url.slice(-4);
    }
    return url;
  }
}
window.maskUrl = maskUrl;

function renderChannelCard(channel) {
  const typeLower = (channel.type || 'webhook').toLowerCase();
  let typeBadge = '';
  if (typeLower === 'discord') {
    typeBadge = `<span class="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-mono font-semibold uppercase tracking-wider bg-[#5865F2]/15 text-[#5865F2] border border-[#5865F2]/30">Discord</span>`;
  } else if (typeLower === 'slack') {
    typeBadge = `<span class="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-mono font-semibold uppercase tracking-wider bg-[#E01E5A]/15 text-[#E01E5A] border border-[#E01E5A]/30">Slack</span>`;
  } else if (typeLower === 'telegram') {
    typeBadge = `<span class="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-mono font-semibold uppercase tracking-wider bg-[#24A1DE]/15 text-[#24A1DE] border border-[#24A1DE]/30">Telegram</span>`;
  } else {
    typeBadge = `<span class="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-mono font-semibold uppercase tracking-wider bg-emerald-500/15 text-emerald-400 border border-emerald-500/30">Custom Webhook</span>`;
  }

  const defaultBadge = channel.is_default
    ? `<span class="inline-flex items-center px-2 py-0.5 rounded text-[9px] font-mono font-semibold uppercase tracking-wider bg-white/10 text-neutral-300 border border-white/15">DEFAULT</span>`
    : '';

  const masked = maskUrl(channel.url);

  return `
    <div class="glass-card rounded-2xl p-5 shadow-xl transition hover:border-white/20 flex flex-col justify-between space-y-4" id="channel-card-${channel.id}">
      <div class="space-y-2">
        <div class="flex items-start justify-between gap-2">
          <div class="space-y-1 min-w-0">
            <h3 class="font-bold text-white text-sm sm:text-base truncate" title="${escapeHtml(channel.name)}">${escapeHtml(channel.name)}</h3>
            <div class="flex items-center gap-1.5 flex-wrap">
              ${typeBadge}
              ${defaultBadge}
            </div>
          </div>
          <!-- Action Menu -->
          <div class="flex items-center gap-1 shrink-0">
            <button data-action="edit-channel" data-id="${channel.id}" class="p-1.5 rounded-lg text-white/50 hover:text-white hover:bg-white/10 transition cursor-pointer" title="Edit Channel">
              <svg class="w-3.5 h-3.5 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"/>
              </svg>
            </button>
            <button data-action="delete-channel" data-id="${channel.id}" data-name="${escapeHtml(channel.name)}" class="p-1.5 rounded-lg text-white/50 hover:text-rose-400 hover:bg-rose-500/10 transition cursor-pointer" title="Delete Channel">
              <svg class="w-3.5 h-3.5 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
              </svg>
            </button>
          </div>
        </div>

        <div class="font-mono text-xs text-white/50 truncate select-all bg-black/30 border border-white/5 rounded-xl px-2.5 py-1.5" title="${escapeHtml(channel.url)}">
          ${escapeHtml(masked)}
        </div>
      </div>

      <div class="pt-2 border-t border-white/5 flex items-center justify-between">
        <span class="text-[10px] font-mono text-white/40">Created ${formatDate(channel.created_at)}</span>
        <button id="test-chan-btn-${channel.id}" data-action="test-channel" data-id="${channel.id}" data-name="${escapeHtml(channel.name)}" class="px-3 py-1 text-xs font-medium rounded-xl bg-white/5 hover:bg-white/10 border border-white/10 text-white/90 hover:text-white transition flex items-center gap-1.5 cursor-pointer">
          <svg class="w-3 h-3 text-[#0a84ff]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"/>
          </svg>
          <span>Test</span>
        </button>
      </div>
    </div>
  `;
}
window.renderChannelCard = renderChannelCard;

function renderNotificationChannels(channels) {
  const container = document.getElementById('channels-list-container');
  if (!container) return;

  if (!channels || channels.length === 0) {
    container.innerHTML = `
      <div class="col-span-full glass-card rounded-2xl p-12 text-center text-white/50 text-xs space-y-3 border border-white/10">
        <div class="w-10 h-10 rounded-full bg-white/5 flex items-center justify-center mx-auto text-white/40">
          <svg class="w-5 h-5 text-[#30d158]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"/>
          </svg>
        </div>
        <p class="font-medium text-white/70">No notification channels registered yet.</p>
        <p class="text-[11px] text-white/40">Add reusable Discord, Slack, Custom Webhook, or Telegram endpoints to receive instant alerts.</p>
        <button onclick="openCreateChannelModal()" class="px-4 py-1.5 rounded-xl bg-[#30d158] hover:bg-[#28b84d] text-[#0b0b0c] text-xs font-semibold shadow-sm hover:shadow-[0_0_15px_rgba(48,209,88,0.35)] transition cursor-pointer">
          + Add First Channel
        </button>
      </div>
    `;
    return;
  }

  container.innerHTML = channels.map((c) => renderChannelCard(c)).join('');
}
window.renderNotificationChannels = renderNotificationChannels;

async function loadNotificationChannels(showLoader = true) {
  if (isChannelsLoading) return;
  isChannelsLoading = true;

  const refreshIcon = document.getElementById('channels-refresh-icon');
  if (refreshIcon && showLoader) {
    refreshIcon.classList.add('animate-spin');
  }

  try {
    const res = await apiFetch('/api/admin/channels');
    if (res && res.ok) {
      const data = await res.json();
      window.cachedChannels = data;
      renderNotificationChannels(data);
    } else {
      console.warn('Failed to fetch notification channels:', res?.status);
    }
  } catch (err) {
    console.error('Error fetching notification channels:', err);
  } finally {
    isChannelsLoading = false;
    if (refreshIcon) {
      refreshIcon.classList.remove('animate-spin');
    }
  }
}
window.loadNotificationChannels = loadNotificationChannels;

async function testNotificationChannel(id, name, btn) {
  if (btn) {
    btn.disabled = true;
    btn.innerHTML = `<span class="inline-block w-3 h-3 border-2 border-white/20 border-t-white rounded-full animate-spin"></span> Testing...`;
  }
  try {
    const res = await apiFetch(`/api/admin/channels/${id}/test`, { method: 'POST' });
    const data = await res.json().catch(() => ({}));
    if (res.ok && data.success !== false) {
      showToast({
        title: 'Channel Probe Delivered',
        message: data.message || `Test notification delivered to "${name}"`,
        type: 'success',
      });
    } else {
      showToast({
        title: 'Channel Test Failed',
        message: data.error || 'Failed to dispatch test notification',
        type: 'error',
      });
    }
  } catch (err) {
    showToast({
      title: 'Channel Test Error',
      message: err.message || 'Network error while testing channel',
      type: 'error',
    });
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.innerHTML = `
        <svg class="w-3 h-3 text-[#0a84ff]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"/>
        </svg>
        <span>Test</span>
      `;
    }
  }
}
window.testNotificationChannel = testNotificationChannel;

async function deleteNotificationChannel(id, name) {
  if (!confirm(`Delete notification channel "${name}"? This channel will be unlinked from all monitors.`)) return;

  try {
    const res = await apiFetch(`/api/admin/channels/${id}`, { method: 'DELETE' });
    if (res && res.ok) {
      showToast({
        title: 'Channel Deleted',
        message: `Notification channel "${name}" removed`,
        type: 'success',
      });
      loadNotificationChannels(false);
      if (typeof loadMonitorChannelsChecklist === 'function') {
        loadMonitorChannelsChecklist();
      }
    } else {
      showToast({
        title: 'Delete Failed',
        message: 'Failed to delete notification channel',
        type: 'error',
      });
    }
  } catch (err) {
    showToast({
      title: 'Error',
      message: err.message || 'Network error while deleting channel',
      type: 'error',
    });
  }
}
window.deleteNotificationChannel = deleteNotificationChannel;

function openCreateChannelModal(isInline = false) {
  isChannelModalInline = isInline;
  const modal = document.getElementById('channel-modal');
  const title = document.getElementById('channel-modal-title');
  const form = document.getElementById('channel-form');
  const idEl = document.getElementById('channel-form-id');

  if (title) title.innerText = 'Add Notification Channel';
  if (idEl) idEl.value = '';
  if (form) form.reset();

  const typeEl = document.getElementById('channel-form-type');
  if (typeEl) typeEl.value = 'discord';

  const notifyResolveEl = document.getElementById('channel-form-notify-resolve');
  if (notifyResolveEl) notifyResolveEl.checked = true;
  const tmplEl = document.getElementById('channel-form-template');
  if (tmplEl) tmplEl.value = '';

  if (modal) modal.classList.remove('hidden');
}
window.openCreateChannelModal = openCreateChannelModal;

function openEditChannelModal(id) {
  isChannelModalInline = false;
  const modal = document.getElementById('channel-modal');
  const title = document.getElementById('channel-modal-title');
  const idEl = document.getElementById('channel-form-id');
  const nameEl = document.getElementById('channel-form-name');
  const typeEl = document.getElementById('channel-form-type');
  const urlEl = document.getElementById('channel-form-url');
  const defaultEl = document.getElementById('channel-form-default');

  const channel = (window.cachedChannels || []).find((c) => c.id === id);
  if (!channel) return;

  if (title) title.innerText = 'Edit Notification Channel';
  if (idEl) idEl.value = channel.id;
  if (nameEl) nameEl.value = channel.name;
  if (typeEl) typeEl.value = (channel.type || 'discord').toLowerCase();
  if (urlEl) urlEl.value = channel.url;
  if (defaultEl) defaultEl.checked = channel.is_default;

  const notifyResolveEl = document.getElementById('channel-form-notify-resolve');
  if (notifyResolveEl) notifyResolveEl.checked = channel.notify_on_resolve !== false;
  const tmplEl = document.getElementById('channel-form-template');
  if (tmplEl) tmplEl.value = channel.custom_template || '';

  if (modal) modal.classList.remove('hidden');
}
window.openEditChannelModal = openEditChannelModal;

function closeChannelModal() {
  const modal = document.getElementById('channel-modal');
  if (modal) modal.classList.add('hidden');
  isChannelModalInline = false;
}
window.closeChannelModal = closeChannelModal;

async function submitChannelForm(e) {
  if (e && typeof e.preventDefault === 'function') {
    e.preventDefault();
  }

  const id = document.getElementById('channel-form-id')?.value?.trim();
  const name = document.getElementById('channel-form-name')?.value?.trim();
  const type = document.getElementById('channel-form-type')?.value?.trim();
  const url = document.getElementById('channel-form-url')?.value?.trim();
  const is_default = document.getElementById('channel-form-default')?.checked || false;
  const notify_on_resolve = document.getElementById('channel-form-notify-resolve') ? document.getElementById('channel-form-notify-resolve').checked : true;
  const custom_template = document.getElementById('channel-form-template')?.value?.trim() || null;

  const saveBtn = document.getElementById('save-channel-btn');
  const originalText = saveBtn ? saveBtn.innerText : 'Save Channel';

  if (!name || !url) {
    showToast({
      title: 'Validation Error',
      message: 'Name and URL are required',
      type: 'error',
    });
    return;
  }

  if (saveBtn) {
    saveBtn.disabled = true;
    saveBtn.innerText = 'Saving...';
  }

  try {
    const endpoint = id ? `/api/admin/channels/${id}` : '/api/admin/channels';
    const method = id ? 'PUT' : 'POST';

    const res = await apiFetch(endpoint, {
      method,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, type, url, is_default, notify_on_resolve, custom_template }),
    });

    if (res && res.ok) {
      const savedChannel = await res.json();
      closeChannelModal();
      showToast({
        title: id ? 'Channel Updated' : 'Channel Created',
        message: `Notification channel "${name}" successfully saved`,
        type: 'success',
      });

      // Refresh channels cache
      await loadNotificationChannels(false);

      // If opened inline from monitor modal, re-render checklist and check this channel
      if (typeof loadMonitorChannelsChecklist === 'function') {
        const currentlyChecked = Array.from(
          document.querySelectorAll('input[name="monitor-channel"]:checked')
        ).map((cb) => cb.value);
        if (!currentlyChecked.includes(savedChannel.id)) {
          currentlyChecked.push(savedChannel.id);
        }
        await loadMonitorChannelsChecklist(currentlyChecked);
      }
    } else {
      const err = await res.json().catch(() => ({}));
      showToast({
        title: 'Save Error',
        message: err.error || 'Failed to save channel',
        type: 'error',
      });
    }
  } catch (err) {
    showToast({
      title: 'Error',
      message: err.message || 'Network error occurred',
      type: 'error',
    });
  } finally {
    if (saveBtn) {
      saveBtn.disabled = false;
      saveBtn.innerText = originalText;
    }
  }
}
window.submitChannelForm = submitChannelForm;

// ── Incidents Management ───────────────────────────────────────────────────

let isAdminIncidentsLoading = false;
async function loadAdminIncidents(showLoader = true) {
  if (isAdminIncidentsLoading) return;
  isAdminIncidentsLoading = true;

  const refreshIcon = document.getElementById('incidents-refresh-icon');
  if (refreshIcon && showLoader) refreshIcon.classList.add('animate-spin');

  try {
    const res = await apiFetch('/api/admin/incidents');
    if (!res.ok) throw new Error('Failed to load incidents: ' + res.status);
    const incidents = await res.json();
    window.cachedIncidents = incidents;
    renderAdminIncidents(incidents);
  } catch (err) {
    if (typeof showToast === 'function') {
      showToast({ title: 'Error', message: err.message, type: 'error' });
    }
  } finally {
    isAdminIncidentsLoading = false;
    if (refreshIcon) refreshIcon.classList.remove('animate-spin');
  }
}
window.loadAdminIncidents = loadAdminIncidents;

function renderAdminIncidents(incidents) {
  const container = document.getElementById('incidents-list-container');
  if (!container) return;

  if (!incidents || incidents.length === 0) {
    container.innerHTML = `
      <div class="glass-card rounded-2xl p-8 text-center text-white/50 text-xs space-y-2 border border-white/5">
        <p class="font-medium text-white/70">No incidents recorded</p>
        <p class="text-white/40">Use "+ Create Incident" above to declare an active disruption or maintenance record.</p>
      </div>
    `;
    return;
  }

  const html = incidents.map((inc) => {
    const isResolved = inc.status === 'RESOLVED';
    const statusBadgeClass = isResolved
      ? 'status-badge-operational'
      : inc.status === 'INVESTIGATING'
      ? 'status-badge-down'
      : 'status-badge-degraded';

    let updatesHtml = '';
    if (Array.isArray(inc.updates) && inc.updates.length > 0) {
      updatesHtml = `
        <div class="mt-3 space-y-2 pl-3 border-l border-white/10 text-xs">
          ${inc.updates.map((u) => `
            <div class="space-y-0.5">
              <div class="flex items-center gap-2">
                <span class="font-semibold text-white/90 uppercase text-[10px]">${escapeHtml(u.status)}</span>
                <span class="text-white/40 font-mono text-[10px]">${escapeHtml(formatDate(u.created_at))}</span>
              </div>
              <p class="text-white/70 text-xs">${escapeHtml(u.message)}</p>
            </div>
          `).join('')}
        </div>
      `;
    }

    return `
      <div class="glass-card rounded-2xl p-5 border border-white/10 space-y-3">
        <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
          <div class="flex items-center gap-2.5 flex-wrap">
            <h3 class="font-semibold text-white text-sm">${escapeHtml(inc.title)}</h3>
            <span class="px-2 py-0.5 rounded-full text-[10px] ${statusBadgeClass} font-medium">${escapeHtml(inc.status)}</span>
            <span class="px-1.5 py-0.2 rounded text-[9px] bg-white/10 text-white/60 font-mono">${escapeHtml(inc.severity)}</span>
          </div>
          <div class="flex items-center gap-2">
            <button data-action="open-update-incident" data-id="${escapeHtml(inc.id)}" class="text-xs px-2.5 py-1 rounded-lg bg-white/10 hover:bg-white/15 text-white transition cursor-pointer">
              + Update
            </button>
            ${!isResolved ? `
              <button data-action="resolve-incident" data-id="${escapeHtml(inc.id)}" class="text-xs px-2.5 py-1 rounded-lg bg-[#30d158]/20 hover:bg-[#30d158]/30 text-[#30d158] transition cursor-pointer">
                Resolve
              </button>
            ` : ''}
            <button data-action="delete-incident" data-id="${escapeHtml(inc.id)}" class="text-xs px-2.5 py-1 rounded-lg bg-red-500/10 hover:bg-red-500/20 text-red-400 transition cursor-pointer">
              Delete
            </button>
          </div>
        </div>
        <div class="text-[11px] text-white/40 font-mono">
          <span>Started: ${escapeHtml(formatDate(inc.started_at))}</span>
          ${inc.resolved_at ? `<span> • Resolved: ${escapeHtml(formatDate(inc.resolved_at))}</span>` : ''}
        </div>
        ${updatesHtml}
      </div>
    `;
  }).join('');

  container.innerHTML = html;
}

function openCreateIncidentModal() {
  const modal = document.getElementById('incident-modal');
  const monitorSelect = document.getElementById('incident-form-monitor');
  if (monitorSelect) {
    monitorSelect.innerHTML = '<option value="">None / System-Wide</option>' +
      (window.allMonitors || []).map((m) => `<option value="${escapeHtml(m.id)}">${escapeHtml(m.name)}</option>`).join('');
  }
  const form = document.getElementById('incident-form');
  if (form) form.reset();
  if (modal) modal.classList.remove('hidden');
}
window.openCreateIncidentModal = openCreateIncidentModal;

function closeIncidentModal() {
  const modal = document.getElementById('incident-modal');
  if (modal) modal.classList.add('hidden');
}
window.closeIncidentModal = closeIncidentModal;

async function submitIncidentForm(e) {
  if (e && typeof e.preventDefault === 'function') e.preventDefault();
  const title = document.getElementById('incident-form-title')?.value?.trim();
  const status = document.getElementById('incident-form-status')?.value;
  const severity = document.getElementById('incident-form-severity')?.value;
  const monitor_id = document.getElementById('incident-form-monitor')?.value || null;
  const message = document.getElementById('incident-form-message')?.value?.trim();

  const saveBtn = document.getElementById('save-incident-btn');
  if (saveBtn) {
    saveBtn.disabled = true;
    saveBtn.innerText = 'Posting...';
  }

  try {
    const res = await apiFetch('/api/admin/incidents', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ title, status, severity, monitor_id, message }),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || 'Failed to post incident');
    }

    closeIncidentModal();
    if (typeof showToast === 'function') {
      showToast({ title: 'Incident Declared', message: `Incident "${title}" posted.`, type: 'success' });
    }
    loadAdminIncidents(false);
  } catch (err) {
    if (typeof showToast === 'function') {
      showToast({ title: 'Incident Error', message: err.message, type: 'error' });
    }
  } finally {
    if (saveBtn) {
      saveBtn.disabled = false;
      saveBtn.innerText = 'Post Incident';
    }
  }
}
window.submitIncidentForm = submitIncidentForm;

function openAddIncidentUpdateModal(id) {
  const modal = document.getElementById('incident-update-modal');
  const idEl = document.getElementById('update-form-incident-id');
  if (idEl) idEl.value = id;
  const msgEl = document.getElementById('update-form-message');
  if (msgEl) msgEl.value = '';
  if (modal) modal.classList.remove('hidden');
}
window.openAddIncidentUpdateModal = openAddIncidentUpdateModal;

function closeIncidentUpdateModal() {
  const modal = document.getElementById('incident-update-modal');
  if (modal) modal.classList.add('hidden');
}
window.closeIncidentUpdateModal = closeIncidentUpdateModal;

async function submitIncidentUpdateForm(e) {
  if (e && typeof e.preventDefault === 'function') e.preventDefault();
  const id = document.getElementById('update-form-incident-id')?.value;
  const status = document.getElementById('update-form-status')?.value;
  const message = document.getElementById('update-form-message')?.value?.trim();

  if (!id || !message) return;

  const saveBtn = document.getElementById('save-update-btn');
  if (saveBtn) {
    saveBtn.disabled = true;
    saveBtn.innerText = 'Posting...';
  }

  try {
    const res = await apiFetch(`/api/admin/incidents/${id}/updates`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ status, message }),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || 'Failed to post update');
    }

    closeIncidentUpdateModal();
    if (typeof showToast === 'function') {
      showToast({ title: 'Update Posted', message: 'Incident update appended.', type: 'success' });
    }
    loadAdminIncidents(false);
  } catch (err) {
    if (typeof showToast === 'function') {
      showToast({ title: 'Update Error', message: err.message, type: 'error' });
    }
  } finally {
    if (saveBtn) {
      saveBtn.disabled = false;
      saveBtn.innerText = 'Post Update';
    }
  }
}
window.submitIncidentUpdateForm = submitIncidentUpdateForm;

async function resolveIncident(id) {
  if (!id) return;
  try {
    const res = await apiFetch(`/api/admin/incidents/${id}/resolve`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message: 'Incident marked as resolved by administrator.' }),
    });
    if (!res.ok) throw new Error('Failed to resolve incident');
    if (typeof showToast === 'function') {
      showToast({ title: 'Resolved', message: 'Incident marked as RESOLVED.', type: 'success' });
    }
    loadAdminIncidents(false);
  } catch (err) {
    if (typeof showToast === 'function') {
      showToast({ title: 'Error', message: err.message, type: 'error' });
    }
  }
}
window.resolveIncident = resolveIncident;

async function deleteIncident(id) {
  if (!id) return;
  if (!confirm('Are you sure you want to delete this incident and all timeline updates?')) return;
  try {
    const res = await apiFetch(`/api/admin/incidents/${id}`, { method: 'DELETE' });
    if (!res.ok) throw new Error('Failed to delete incident');
    if (typeof showToast === 'function') {
      showToast({ title: 'Deleted', message: 'Incident deleted.', type: 'success' });
    }
    loadAdminIncidents(false);
  } catch (err) {
    if (typeof showToast === 'function') {
      showToast({ title: 'Error', message: err.message, type: 'error' });
    }
  }
}
window.deleteIncident = deleteIncident;

// ── JSON Configuration Export & Import ─────────────────────────────────────

async function exportConfigJson() {
  try {
    const res = await apiFetch('/api/admin/export');
    if (!res.ok) throw new Error('Export failed: ' + res.status);
    const data = await res.json();
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `keepup-export-${new Date().toISOString().slice(0, 10)}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    if (typeof showToast === 'function') {
      showToast({ title: 'Export Complete', message: 'KeepUp configuration exported successfully.', type: 'success' });
    }
  } catch (err) {
    if (typeof showToast === 'function') {
      showToast({ title: 'Export Failed', message: err.message, type: 'error' });
    }
  }
}
window.exportConfigJson = exportConfigJson;

async function importConfigJson(inputEl) {
  const file = inputEl?.files?.[0];
  if (!file) return;
  try {
    const text = await file.text();
    const payload = JSON.parse(text);
    const res = await apiFetch('/api/admin/import', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || 'Import failed');
    }
    const result = await res.json();
    if (typeof showToast === 'function') {
      showToast({
        title: 'Import Successful',
        message: `Imported ${result.imported_monitors} monitors, ${result.imported_channels} channels.`,
        type: 'success',
      });
    }
    inputEl.value = '';
    fetchDashboardData(true);
  } catch (err) {
    if (typeof showToast === 'function') {
      showToast({ title: 'Import Failed', message: err.message, type: 'error' });
    }
    inputEl.value = '';
  }
}
window.importConfigJson = importConfigJson;

// ── Initialization & Auto-Refresh Loop (Every 5 seconds) ─────────────────────

document.addEventListener('DOMContentLoaded', () => {
  const badgeBackdrop = document.getElementById('badge-modal');
  if (badgeBackdrop) {
    badgeBackdrop.addEventListener('click', (e) => {
      if (e.target === badgeBackdrop) {
        closeBadgeModal();
      }
    });
  }

  const passwordBackdrop = document.getElementById('password-modal');
  if (passwordBackdrop) {
    passwordBackdrop.addEventListener('click', (e) => {
      if (e.target === passwordBackdrop) {
        closePasswordModal();
      }
    });
  }

  const detailBackdrop = document.getElementById('monitor-detail-modal');
  if (detailBackdrop) {
    detailBackdrop.addEventListener('click', (e) => {
      if (e.target === detailBackdrop) {
        closeMonitorDetailModal();
      }
    });
  }

  const channelBackdrop = document.getElementById('channel-modal');
  if (channelBackdrop) {
    channelBackdrop.addEventListener('click', (e) => {
      if (e.target === channelBackdrop) {
        closeChannelModal();
      }
    });
  }

  const incidentBackdrop = document.getElementById('incident-modal');
  if (incidentBackdrop) {
    incidentBackdrop.addEventListener('click', (e) => {
      if (e.target === incidentBackdrop) {
        closeIncidentModal();
      }
    });
  }

  const updateBackdrop = document.getElementById('incident-update-modal');
  if (updateBackdrop) {
    updateBackdrop.addEventListener('click', (e) => {
      if (e.target === updateBackdrop) {
        closeIncidentUpdateModal();
      }
    });
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      const channelModal = document.getElementById('channel-modal');
      if (channelModal && !channelModal.classList.contains('hidden')) {
        closeChannelModal();
      }
      const incModal = document.getElementById('incident-modal');
      if (incModal && !incModal.classList.contains('hidden')) {
        closeIncidentModal();
      }
      const upModal = document.getElementById('incident-update-modal');
      if (upModal && !upModal.classList.contains('hidden')) {
        closeIncidentUpdateModal();
      }
    }
  });

  const isAuthOrStatusPage =
    window.location.pathname.includes('login') ||
    window.location.pathname.includes('status');

  function initLiveEventStream() {
    if (window.EventSource) {
      try {
        const evtSource = new EventSource('/api/events');
        evtSource.onmessage = (event) => {
          try {
            const data = JSON.parse(event.data);
            if (data.event === 'check_completed' || data.event === 'status_changed') {
              if (!document.hidden && currentAdminView === 'monitors') {
                fetchDashboardData(false);
              }
            }
          } catch (_) {}
        };
        evtSource.onerror = () => {
          // EventSource will auto-reconnect
        };
      } catch (_) {}
    }
  }

  if (!isAuthOrStatusPage) {
    setViewMode(window.currentViewMode);
    fetchDashboardData(true);
    initLiveEventStream();

    // Auto-refresh fallback every 10 seconds (paused when tab hidden)
    setInterval(() => {
      if (document.hidden) return;
      if (currentAdminView === 'assets') {
        loadAssetExpiryData(false);
      } else if (currentAdminView === 'channels') {
        loadNotificationChannels(false);
      } else if (currentAdminView === 'incidents') {
        loadAdminIncidents(false);
      } else {
        fetchDashboardData(false);
      }
    }, 10000);
  }
});
