// ── KeepUp Main Dashboard Controller (Apple HIG Aesthetics) ──────────────

// Global apiFetch wrapper fallback with 401 redirect loop guard
if (!window.apiFetch) {
  window.apiFetch = async function apiFetch(url, options = {}) {
    options.credentials = options.credentials || 'include';
    const res = await fetch(url, options);
    if (res.status === 401) {
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

// ── Dashboard Data Fetching ──────────────────────────────────────────────────

async function fetchDashboardData(showSpin = false) {
  if (isFetching) return;
  isFetching = true;

  const refreshIcon = document.getElementById('refresh-icon');
  if (showSpin && refreshIcon) refreshIcon.classList.add('animate-spin');

  try {
    const [monitorsRes, statusRes, webhooksRes] = await Promise.all([
      apiFetch('/api/monitors'),
      apiFetch('/api/v1/public/status'),
      apiFetch('/api/webhooks'),
    ]);

    // If unauthorized, do not render dashboard contents (redirect is underway)
    if (monitorsRes && monitorsRes.status === 401) {
      const isLoginPage =
        window.location.pathname.endsWith('/login.html') ||
        window.location.pathname === '/login';
      if (!isLoginPage) {
        window.location.href = '/login.html';
      }
      return;
    }

    if (monitorsRes && monitorsRes.ok) {
      window.allMonitors = await monitorsRes.json();
    }

    if (statusRes.ok) {
      const statusData = await statusRes.json();
      statusMap = {};
      (statusData.monitors || []).forEach((m) => {
        statusMap[m.id] = m;
      });
      updateOverviewHero(statusData);
    }

    if (webhooksRes.ok) {
      window.allWebhooks = await webhooksRes.json();
      renderWebhooksSection(window.allWebhooks);
    }

    // Fetch recent check logs for historical bar charts (top 30 per monitor)
    await fetchRecentLogsForMonitors(window.allMonitors);

    renderMonitors();
  } catch (err) {
    console.error('Failed to fetch dashboard data:', err);
  } finally {
    isFetching = false;
    if (showSpin && refreshIcon) {
      setTimeout(() => refreshIcon.classList.remove('animate-spin'), 300);
    }
  }
}

// ── Recent Logs Batching for Historical Bar Chart ────────────────────────────

async function fetchRecentLogsForMonitors(monitors) {
  const fetchPromises = monitors.map(async (m) => {
    try {
      const res = await apiFetch(`/api/v1/monitors/${m.id}/logs?limit=30`);
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
    container.innerHTML = `
      <div class="apple-glass rounded-2xl p-12 text-center text-white/50 space-y-3">
        <svg class="w-10 h-10 mx-auto text-white/30" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
        </svg>
        <p class="text-sm font-medium text-white/70">${query ? 'No matching targets found' : 'No monitors configured yet'}</p>
        <p class="text-xs text-white/40">${query ? 'Try searching for a different keyword' : 'Click "+ Add Monitor" to create your first health check target'}</p>
      </div>
    `;
    return;
  }

  if (window.currentViewMode === 'grid') {
    container.className = 'grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4';
    container.innerHTML = filtered.map((m) => renderMonitorCard(m)).join('');
  } else {
    container.className = 'space-y-2.5';
    container.innerHTML = filtered.map((m) => renderMonitorListRow(m)).join('');
  }
}

// ── Single Monitor Card (Grid View) ──────────────────────────────────────────

function renderMonitorCard(m) {
  const live = statusMap[m.id];
  const isUp = live ? live.current_status === 'UP' : true;
  const isPaused = m.is_active === 0;

  let statusColor = '#30d158'; // Apple Green
  let statusBadgeBg = 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 border border-emerald-500/20';
  let statusLabel = 'OPERATIONAL';

  if (isPaused) {
    statusColor = '#8e8e93'; // Apple Gray
    statusBadgeBg = 'bg-neutral-500/15 text-neutral-600 dark:text-neutral-400 border border-neutral-500/20';
    statusLabel = 'PAUSED';
  } else if (!isUp) {
    statusColor = '#ff453a'; // Apple Red
    statusBadgeBg = 'bg-rose-500/15 text-rose-700 dark:text-rose-400 border border-rose-500/20';
    statusLabel = 'DOWN';
  } else if (live && (live.current_status === 'DEGRADED' || (live.uptime_24h !== undefined && live.uptime_24h < 95.0))) {
    statusColor = '#ffd60a'; // Apple Yellow
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
    <div class="apple-glass rounded-2xl p-5 shadow-xl transition-all duration-200 hover:border-black/15 dark:hover:border-white/20 flex flex-col justify-between space-y-4">
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
          <button onclick="toggleMonitorActive('${m.id}', ${m.is_active === 1})" class="p-1.5 rounded-lg text-neutral-500 hover:text-neutral-900 dark:text-white/60 dark:hover:text-white hover:bg-black/5 dark:hover:bg-white/10 transition" title="${m.is_active === 1 ? 'Pause Monitor' : 'Resume Monitor'}">
            ${
              m.is_active === 1
                ? `<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 9v6m4-6v6m7-3a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>`
                : `<svg class="w-4 h-4 text-[#30d158]" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z"/><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>`
            }
          </button>
          <button onclick="openEditMonitorModal('${m.id}')" class="px-2.5 py-1 rounded-lg bg-black/5 hover:bg-black/10 dark:bg-white/5 dark:hover:bg-white/10 text-neutral-700 hover:text-neutral-900 dark:text-white/80 dark:hover:text-white font-medium border border-black/5 dark:border-white/10 transition" title="Edit Target">
            Edit
          </button>
          <button onclick="viewLogs('${m.id}', '${escapeHtml(m.name)}', '${escapeHtml(m.url)}')" class="px-2.5 py-1 rounded-lg bg-black/5 hover:bg-black/10 dark:bg-white/5 dark:hover:bg-white/10 text-neutral-700 hover:text-neutral-900 dark:text-white/80 dark:hover:text-white font-medium border border-black/5 dark:border-white/10 transition" title="View History Logs">
            Logs
          </button>
          <button onclick="deleteMonitor('${m.id}', '${escapeHtml(m.name)}')" class="p-1.5 rounded-lg text-neutral-400 hover:text-[#ff453a] hover:bg-[#ff453a]/10 transition" title="Delete Target">
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
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
    <div class="apple-glass rounded-2xl p-4 shadow-lg flex flex-col sm:flex-row sm:items-center justify-between gap-3 hover:border-black/15 dark:hover:border-white/20 transition">
      <div class="flex items-center gap-3 min-w-0">
        <span class="relative flex h-2.5 w-2.5 shrink-0">
          <span class="relative inline-flex rounded-full h-2.5 w-2.5" style="background-color: ${statusColor}"></span>
        </span>
        <div class="min-w-0">
          <div class="flex items-center gap-2">
            <span class="font-semibold text-neutral-900 dark:text-white text-sm truncate">${escapeHtml(m.name)}</span>
            <span class="px-2 py-0.5 rounded-full text-[10px] font-medium border uppercase ${statusBadgeBg}">${statusLabel}</span>
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
          <button onclick="toggleMonitorActive('${m.id}', ${m.is_active === 1})" class="p-1.5 rounded-lg text-neutral-500 hover:text-neutral-900 dark:text-white/60 dark:hover:text-white hover:bg-black/5 dark:hover:bg-white/10 transition" title="Toggle Pause">
            ${m.is_active === 1 ? '⏸' : '▶'}
          </button>
          <button onclick="openEditMonitorModal('${m.id}')" class="px-2.5 py-1 rounded-lg bg-black/5 hover:bg-black/10 dark:bg-white/5 dark:hover:bg-white/10 text-neutral-700 hover:text-neutral-900 dark:text-white/80 dark:hover:text-white border border-black/5 dark:border-white/10 font-medium transition">Edit</button>
          <button onclick="viewLogs('${m.id}', '${escapeHtml(m.name)}', '${escapeHtml(m.url)}')" class="px-2.5 py-1 rounded-lg bg-black/5 hover:bg-black/10 dark:bg-white/5 dark:hover:bg-white/10 text-neutral-700 hover:text-neutral-900 dark:text-white/80 dark:hover:text-white border border-black/5 dark:border-white/10 font-medium transition">Logs</button>
          <button onclick="deleteMonitor('${m.id}', '${escapeHtml(m.name)}')" class="p-1.5 text-neutral-400 hover:text-[#ff453a] hover:bg-[#ff453a]/10 rounded-lg transition">✕</button>
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

function renderWebhooksSection(webhooks) {
  const container = document.getElementById('webhooks-preview-list');
  if (!container) return;

  if (webhooks.length === 0) {
    container.innerHTML = `
      <div class="col-span-full p-6 text-center text-neutral-500 dark:text-white/40 text-xs apple-glass rounded-xl">
        No notification webhooks registered. Click "+ Add Webhook" to receive instant alert cards.
      </div>
    `;
    return;
  }

  container.innerHTML = webhooks
    .map((w) => {
      const isEnabled = w.is_enabled === 1 || w.is_enabled === true;
      const providerColor =
        w.provider.toLowerCase() === 'discord'
          ? 'text-[#5865F2] border-[#5865F2]/30 bg-[#5865F2]/10'
          : w.provider.toLowerCase() === 'slack'
          ? 'text-[#ECB22E] border-[#ECB22E]/30 bg-[#ECB22E]/10'
          : 'text-[#30d158] border-[#30d158]/30 bg-[#30d158]/10';

      return `
      <div class="apple-glass rounded-2xl p-4 flex items-center justify-between gap-3 shadow-lg hover:border-black/15 dark:hover:border-white/20 transition">
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
          <button onclick="testWebhookDirect('${w.id}', '${escapeHtml(w.name)}')" class="px-2.5 py-1 text-xs font-medium rounded-lg bg-black/5 hover:bg-black/10 dark:bg-white/5 dark:hover:bg-white/10 text-neutral-700 hover:text-neutral-900 dark:text-white/80 dark:hover:text-white border border-black/5 dark:border-white/10 transition" title="Test Notification">
            Test
          </button>
          <!-- Apple-style toggle -->
          <label class="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" ${isEnabled ? 'checked' : ''} onchange="toggleWebhookActive('${w.id}', ${isEnabled})" class="sr-only peer">
            <div class="w-9 h-5 bg-neutral-300 dark:bg-neutral-700 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-[#30d158]"></div>
          </label>
        </div>
      </div>
    `;
    })
    .join('');
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
    const res = await apiFetch(`/api/webhooks/${id}`, {
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
    const res = await apiFetch(`/api/webhooks/${id}/test`, { method: 'POST' });
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

function formatDate(str) {
  if (!str) return '—';
  try {
    const d = new Date(str.includes('Z') ? str : str + 'Z');
    return (
      d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }) +
      ' ' +
      d.toLocaleDateString([], { month: 'short', day: 'numeric' })
    );
  } catch {
    return str;
  }
}

// showToast is provided globally by static/toast.js

// ── Initialization & Auto-Refresh Loop (Every 5 seconds) ─────────────────────

document.addEventListener('DOMContentLoaded', () => {
  const isAuthOrStatusPage =
    window.location.pathname.includes('login') ||
    window.location.pathname.includes('status');

  if (!isAuthOrStatusPage) {
    setViewMode(window.currentViewMode);
    fetchDashboardData(true);

    // Auto-refresh every 5 seconds per PRD requirements
    setInterval(() => {
      fetchDashboardData(false);
    }, 5000);
  }
});
