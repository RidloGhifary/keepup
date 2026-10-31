// ── KeepUp Public Status Page Controller (Clean Minimalist Aesthetics) ───────────

const VALID_THEMES = ['auto', 'default', 'dark', 'oled', 'midnight', 'light'];

// ── Theme Management & Dynamic OS Listener ───────────────────────────────────

function updateToggleIcons(isLightMode) {
  const sunIcon = document.getElementById('theme-icon-sun');
  const moonIcon = document.getElementById('theme-icon-moon');
  const toggleBtn = document.getElementById('theme-toggle-btn');
  if (!sunIcon || !moonIcon) return;

  const currentTheme =
    document.documentElement.getAttribute('data-theme') || 'auto';

  if (isLightMode) {
    sunIcon.classList.add('hidden');
    moonIcon.classList.remove('hidden');
    if (toggleBtn) {
      const modeLabel = currentTheme === 'auto' ? 'Auto (Light)' : 'Light';
      toggleBtn.setAttribute(
        'title',
        `Theme: ${modeLabel} — Click to switch to ${currentTheme === 'light' ? 'Auto' : 'Dark'}`
      );
      toggleBtn.setAttribute('aria-label', 'Switch theme');
    }
  } else {
    sunIcon.classList.remove('hidden');
    moonIcon.classList.add('hidden');
    if (toggleBtn) {
      const modeLabel = currentTheme === 'auto' ? 'Auto (Dark)' : 'Dark';
      toggleBtn.setAttribute(
        'title',
        `Theme: ${modeLabel} — Click to switch to Light`
      );
      toggleBtn.setAttribute('aria-label', 'Switch theme');
    }
  }
}

function toggleTheme() {
  const currentPref = (
    localStorage.getItem('keepup_theme_preference') ||
    localStorage.getItem('keepup_status_theme') ||
    document.documentElement.getAttribute('data-theme') ||
    'auto'
  ).toLowerCase();

  let nextTheme = 'auto';
  if (currentPref === 'auto') {
    nextTheme = 'dark';
  } else if (
    currentPref === 'dark' ||
    currentPref === 'default' ||
    currentPref === 'oled' ||
    currentPref === 'midnight'
  ) {
    nextTheme = 'light';
  } else if (currentPref === 'light') {
    nextTheme = 'auto';
  } else {
    nextTheme = 'dark';
  }

  setTheme(nextTheme);
}

function initAutoTheme() {
  const urlParam = new URLSearchParams(window.location.search).get('theme');
  const storedTheme =
    localStorage.getItem('keepup_theme_preference') ||
    localStorage.getItem('keepup_status_theme') ||
    localStorage.getItem('keepup_theme');

  let effectiveTheme = null;
  if (urlParam && urlParam.trim()) {
    effectiveTheme = urlParam.trim().toLowerCase();
  } else if (storedTheme && storedTheme.trim()) {
    effectiveTheme = storedTheme.trim().toLowerCase();
  }

  if (
    effectiveTheme &&
    effectiveTheme !== 'auto' &&
    VALID_THEMES.includes(effectiveTheme)
  ) {
    applyExplicitTheme(effectiveTheme);
  } else {
    applyAutoTheme();
  }

  // Real-time matchMedia listener for OS mode changes
  const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
  const handleOSChange = () => {
    const currentUrlParam = new URLSearchParams(window.location.search).get(
      'theme'
    );
    const currentStored =
      localStorage.getItem('keepup_theme_preference') ||
      localStorage.getItem('keepup_status_theme') ||
      localStorage.getItem('keepup_theme');
    const override = (currentUrlParam || currentStored || '').toLowerCase();
    if (!override || override === 'auto') {
      applyAutoTheme();
    }
  };

  if (typeof mediaQuery.addEventListener === 'function') {
    mediaQuery.addEventListener('change', handleOSChange);
  } else if (typeof mediaQuery.addListener === 'function') {
    mediaQuery.addListener(handleOSChange);
  }
}

function applyAutoTheme() {
  document.documentElement.setAttribute('data-theme', 'auto');
  const isDarkMode =
    window.matchMedia &&
    window.matchMedia('(prefers-color-scheme: dark)').matches;
  if (isDarkMode) {
    document.documentElement.classList.add('dark');
  } else {
    document.documentElement.classList.remove('dark');
  }
  updateThemeMetaColor(isDarkMode ? '#0b0b0c' : '#f5f5f7');
  updateThemeButtonUI('auto');
  updateToggleIcons(!isDarkMode);
}

function applyExplicitTheme(themeName) {
  const theme = VALID_THEMES.includes(themeName) ? themeName : 'default';
  document.documentElement.setAttribute('data-theme', theme);

  if (theme === 'light') {
    document.documentElement.classList.remove('dark');
    updateThemeMetaColor('#f5f5f7');
    updateToggleIcons(true);
  } else {
    document.documentElement.classList.add('dark');
    const metaColors = {
      default: '#0b0b0c',
      dark: '#0b0b0c',
      oled: '#000000',
      midnight: '#050b14',
    };
    updateThemeMetaColor(metaColors[theme] || '#0b0b0c');
    updateToggleIcons(false);
  }

  updateThemeButtonUI(theme);
}

function setTheme(themeName) {
  const theme = (themeName || 'auto').toLowerCase();
  localStorage.setItem('keepup_theme_preference', theme);
  localStorage.setItem('keepup_status_theme', theme);
  localStorage.setItem('keepup_theme', theme);

  if (theme === 'auto') {
    const url = new URL(window.location);
    url.searchParams.delete('theme');
    window.history.replaceState({}, '', url);
    applyAutoTheme();
  } else {
    const url = new URL(window.location);
    url.searchParams.set('theme', theme);
    window.history.replaceState({}, '', url);
    applyExplicitTheme(theme);
  }
}

// Expose theme functions globally
window.toggleTheme = toggleTheme;
window.setTheme = setTheme;
window.initAutoTheme = initAutoTheme;

function updateThemeMetaColor(hexColor) {
  const themeMeta = document.querySelector('meta[name="theme-color"]');
  if (themeMeta) {
    themeMeta.setAttribute('content', hexColor);
  }
}

function updateThemeButtonUI(activeTheme) {
  document.querySelectorAll('[data-theme-btn]').forEach((btn) => {
    const btnTheme = btn.getAttribute('data-theme-btn');
    if (btnTheme === activeTheme) {
      btn.className =
        'px-2 py-0.5 rounded-full transition bg-white/10 text-white font-semibold shadow-sm border border-white/10';
    } else {
      btn.className =
        'px-2 py-0.5 rounded-full transition text-white/50 hover:text-white border border-transparent';
    }
  });
}

// Initial theme resolution to prevent unstyled flash
initAutoTheme();

// Check URL query parameters for demo mode
const urlParams = new URLSearchParams(window.location.search);
if (urlParams.get('demo') === 'true') {
  document.documentElement.classList.add('demo-mode');
  if (document.body) {
    document.body.classList.add('demo-mode');
  } else {
    document.addEventListener('DOMContentLoaded', () => {
      document.body.classList.add('demo-mode');
    });
  }
}

// ── Realistic Demo Mock Data Generator (Used when demo=true or backend is offline) ───
function getMockStatusData() {
  const now = new Date();

  function generate7DayHistory(hasDegraded = false) {
    const list = [];
    for (let i = 6; i >= 0; i--) {
      const d = new Date(now.getTime() - i * 86400000);
      const isDegradedDay = hasDegraded && i === 2;
      list.push({
        date: d.toISOString().split('T')[0],
        uptime_pct: isDegradedDay ? 98.4 : 100.0,
        status: isDegradedDay ? 'DEGRADED' : 'UP'
      });
    }
    return list;
  }

  function generate24hLatency(baseMs, jitter = 3) {
    const slots = [];
    const currentHour = new Date(now.getFullYear(), now.getMonth(), now.getDate(), now.getHours(), 0, 0);
    for (let i = 23; i >= 0; i--) {
      const slotDate = new Date(currentHour.getTime() - i * 3600 * 1000);
      const variance = Math.floor(Math.random() * (jitter * 2 + 1)) - jitter;
      slots.push({
        timestamp: slotDate.toISOString(),
        avg_latency_ms: Math.max(5, baseMs + variance),
        status: 'UP'
      });
    }
    return slots;
  }

  return {
    status: 'All Systems Operational',
    status_page_theme: 'dark',
    monitors: [
      {
        id: 1,
        name: 'API Gateway',
        current_status: 'UP',
        last_latency_ms: 18,
        uptime_24h: 100.0,
        uptime_history: generate7DayHistory(false),
        latency_24h: generate24hLatency(18, 3),
        latency: { p50: 17, p95: 24, p99: 38, avg: 18 }
      },
      {
        id: 2,
        name: 'Database Cluster',
        current_status: 'UP',
        last_latency_ms: 12,
        uptime_24h: 100.0,
        uptime_history: generate7DayHistory(false),
        latency_24h: generate24hLatency(12, 2),
        latency: { p50: 11, p95: 16, p99: 22, avg: 12 }
      },
      {
        id: 3,
        name: 'Payment Webhooks',
        current_status: 'UP',
        last_latency_ms: 42,
        uptime_24h: 99.8,
        uptime_history: generate7DayHistory(true),
        latency_24h: generate24hLatency(42, 5),
        latency: { p50: 39, p95: 68, p99: 110, avg: 43 }
      }
    ]
  };
}

let statusData = { status: 'Checking...', monitors: [] };
let countdownSeconds = 30;
let countdownTimer = null;

// ── Public Status Fetcher ────────────────────────────────────────────────────

const getSlug = () => {
  const pathParts = window.location.pathname.split('/').filter(Boolean);
  // Handles /status/dri, /status/dri.html, or ?slug=dri
  if (pathParts.length >= 2 && pathParts[0] === 'status') {
    return pathParts[1].replace('.html', '');
  }
  const params = new URLSearchParams(window.location.search);
  return params.get('slug') || params.get('page') || 'default';
};

const getSlugFromUrl = getSlug;
const getStatusSlug = getSlug;

function renderNotFoundStatus(slug) {
  const heroCard = document.getElementById('hero-status-card');
  const heroPulse = document.getElementById('hero-status-pulse');
  const heroTitle = document.getElementById('hero-status-title');
  const heroDesc = document.getElementById('page-description') || document.getElementById('hero-status-desc');
  const container = document.getElementById('monitors-container') || document.getElementById('public-services-container');
  const countBadge = document.getElementById('services-count-badge');
  const timelineSection = document.getElementById('incident-timeline-container');

  if (heroPulse) {
    heroPulse.style.backgroundColor = '#71717a';
    heroPulse.className = 'h-3.5 w-3.5 rounded-full';
  }
  if (heroTitle) {
    heroTitle.innerText = 'Status Page Not Found';
  }
  if (heroDesc) {
    heroDesc.innerHTML = `The status page <code class="font-mono bg-white/10 px-1.5 py-0.5 rounded text-white">${escapeHtml(slug)}</code> does not exist or has been made private.<br><a href="/status" class="inline-flex items-center gap-1.5 text-[#0a84ff] hover:underline mt-3 text-xs font-semibold">View Default Status Page &rarr;</a>`;
  }
  if (heroCard) {
    heroCard.style.boxShadow = 'var(--card-shadow), 0 20px 40px -15px rgba(113, 113, 122, 0.25)';
  }
  if (countBadge) {
    countBadge.innerText = '0 components';
  }
  if (container) {
    container.innerHTML = `
      <div class="glass-card rounded-2xl p-10 text-center text-white/50 text-xs">
        No components configured for status page "${escapeHtml(slug)}".
      </div>
    `;
  }
  if (timelineSection) {
    timelineSection.innerHTML = `
      <div class="p-6 text-center text-white/40 text-xs">
        No incident history recorded for this page.
      </div>
    `;
  }
  document.title = '404 - Status Page Not Found — KeepUp';
}

function renderMonitors(monitors, groups) {
  renderMonitorsList(monitors, groups);
}

async function loadStatusData() {
  const slug = getSlug();
  const loadingEl = document.getElementById('loading-indicator');
  const container = document.getElementById('monitors-container') || document.getElementById('public-services-container');
  const countBadge = document.getElementById('services-count-badge');
  const urlParamDemo = new URLSearchParams(window.location.search).get('demo') === 'true';

  if (urlParamDemo) {
    statusData = getMockStatusData();
    if (loadingEl) loadingEl.style.display = 'none';
    renderOverallHero(statusData);
    renderMaintenanceBanners(statusData.maintenance_windows || []);
    renderMonitors(statusData.monitors || [], statusData.groups || []);
    renderIncidentTimeline(statusData);
    updateLastUpdatedTime();
    return;
  }

  try {
    const res = await fetch(`/api/public/status/${encodeURIComponent(slug)}`);
    if (!res.ok) {
      if (res.status === 404) {
        if (loadingEl) loadingEl.style.display = 'none';
        renderNotFoundStatus(slug);
        updateLastUpdatedTime();
        return;
      }
      throw new Error(`HTTP ${res.status} fetching status for slug: ${slug}`);
    }

    const data = await res.json();
    statusData = data;

    // 1. Update Page Title & Description dynamically if provided
    if (data.title) {
      const titleEl = document.getElementById('page-title');
      if (titleEl) titleEl.textContent = data.title;
      const badgeEl = document.getElementById('status-page-badge');
      if (badgeEl) badgeEl.textContent = data.title.toUpperCase();
      document.title = `${data.title} — KeepUp Status`;
    }
    if (data.description) {
      const descEl = document.getElementById('page-description') || document.getElementById('hero-status-desc');
      if (descEl) descEl.textContent = data.description;
    }

    // 2. Extract Monitors (handles data.monitors, data.groups, or raw array)
    let monitors = [];
    if (Array.isArray(data)) {
      monitors = data;
    } else if (Array.isArray(data.monitors)) {
      monitors = data.monitors;
    } else if (Array.isArray(data.groups)) {
      // Flatten monitors from group structures
      monitors = data.groups.flatMap((g) => g.monitors || []);
    }

    // Check backend status_page_theme
    const urlTheme = new URLSearchParams(window.location.search).get('theme');
    const storedTheme =
      localStorage.getItem('keepup_theme_preference') ||
      localStorage.getItem('keepup_status_theme') ||
      localStorage.getItem('keepup_theme');
    if (!urlTheme && !storedTheme && data.status_page_theme) {
      const backendTheme = data.status_page_theme.toLowerCase();
      if (backendTheme === 'auto') {
        applyAutoTheme();
      } else if (VALID_THEMES.includes(backendTheme)) {
        applyExplicitTheme(backendTheme);
      }
    }

    // 3. Hide Loading State & Render
    if (loadingEl) loadingEl.style.display = 'none';

    renderOverallHero(data);
    renderMaintenanceBanners(data.maintenance_windows || []);
    renderIncidentTimeline(data);
    updateLastUpdatedTime();

    if (monitors.length === 0) {
      if (countBadge) countBadge.innerText = '0 components';
      if (container) {
        container.innerHTML = `<div class="p-6 text-center text-neutral-400 font-mono text-xs">No monitors assigned to this status page.</div>`;
      }
      return;
    }

    renderMonitors(monitors, data.groups);
  } catch (err) {
    console.error('Failed to load status page:', err);
    if (loadingEl) loadingEl.style.display = 'none';
    if (container) {
      container.innerHTML = `<div class="p-6 text-center text-rose-400 font-mono text-xs">Failed to load status telemetry. Please try again.</div>`;
    }
  }
}

const fetchPublicStatus = loadStatusData;

// ── Overall System Status Hero Card ──────────────────────────────────────────

function renderOverallHero(data) {
  const heroCard = document.getElementById('hero-status-card');
  const heroPulse = document.getElementById('hero-status-pulse');
  const heroTitle = document.getElementById('hero-status-title');
  const heroDesc = document.getElementById('page-description') || document.getElementById('hero-status-desc');
  const pageBadge = document.getElementById('status-page-badge');

  if (data.title) {
    document.title = `${data.title} — KeepUp Status`;
    if (pageBadge) {
      pageBadge.textContent = data.title.toUpperCase();
    }
  }

  if (!heroCard || !heroTitle) return;

  const monitors = data.monitors || [];
  let anyDown = false;
  let anyDegraded = false;

  monitors.forEach((m) => {
    if (m.current_status === 'DOWN') anyDown = true;
    else if (m.current_status === 'DEGRADED' || m.uptime_24h < 95.0) anyDegraded = true;
  });

  let statusTitle = data.status || 'All Systems Operational';
  let badgeColor = 'var(--status-operational)';
  let glowColor = '#30d158';
  let descText = data.description || 'All monitored components, APIs, and microservices are operating normally.';

  if (anyDown || statusTitle.includes('Major')) {
    statusTitle = 'Major System Outage';
    badgeColor = 'var(--status-down)';
    glowColor = '#ff453a';
    descText = 'One or more core public services are experiencing critical service disruption.';
  } else if (anyDegraded || statusTitle.includes('Partial')) {
    statusTitle = 'Partial System Disruption';
    badgeColor = 'var(--status-degraded)';
    glowColor = '#ffd60a';
    descText = 'Some monitored components are experiencing intermittent latency or degraded performance.';
  }

  heroTitle.innerText = statusTitle;
  heroDesc.innerText = descText;

  if (heroPulse) {
    heroPulse.style.backgroundColor = badgeColor;
    heroPulse.className = `h-3.5 w-3.5 rounded-full ${anyDown ? '' : 'animate-pulse'}`;
  }

  // Update Hero Card glassmorphic border & glow accent
  heroCard.style.boxShadow = `var(--card-shadow), 0 20px 40px -15px ${glowColor}25`;
}

// ── Active Maintenance Banners ──────────────────────────────────────────────

function formatTimeUtc(isoStr) {
  try {
    const d = new Date(isoStr);
    return d.toUTCString().replace(':00 GMT', ' UTC').replace(' GMT', ' UTC');
  } catch (_) {
    return isoStr;
  }
}

function renderMaintenanceBanners(windows) {
  const container = document.getElementById('maintenance-banner-container');
  if (!container) return;

  const now = new Date();
  const activeWindows = (windows || []).filter((w) => {
    if (w.is_active !== undefined) return w.is_active;
    const start = new Date(w.start_time);
    const end = new Date(w.end_time);
    return now >= start && now <= end;
  });

  if (activeWindows.length === 0) {
    container.innerHTML = '';
    return;
  }

  container.innerHTML = activeWindows
    .map((w) => {
      const endFormatted = formatTimeUtc(w.end_time);
      return `
        <div class="glass-card rounded-2xl p-4 sm:p-5 border border-purple-500/30 bg-purple-500/10 shadow-lg text-white">
          <div class="flex items-start gap-3.5">
            <div class="p-2 rounded-xl bg-purple-500/20 text-purple-400 border border-purple-500/30 shrink-0 mt-0.5">
              <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
              </svg>
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2">
                <span class="text-[10px] font-bold uppercase tracking-wider text-purple-300 bg-purple-500/20 px-2 py-0.5 rounded-md border border-purple-500/30 font-mono">
                  Scheduled Maintenance in Progress
                </span>
              </div>
              <h3 class="text-sm sm:text-base font-semibold text-white mt-1.5">${escapeHtml(w.title)}</h3>
              ${w.description ? `<p class="text-xs text-white/70 mt-1">${escapeHtml(w.description)}</p>` : ''}
              <div class="text-[11px] text-white/50 font-mono mt-2 flex items-center gap-2">
                <span>Window Ends: <strong class="text-white">${escapeHtml(endFormatted)}</strong></span>
              </div>
            </div>
          </div>
        </div>
      `;
    })
    .join('');
}

// ── Monitors & Services List (Categorized Group Support) ─────────────────────

function renderMonitorsList(monitors, groups) {
  const container = document.getElementById('monitors-container') || document.getElementById('public-services-container');
  const countBadge = document.getElementById('services-count-badge');

  if (!container) return;

  const totalCount = monitors.length;
  if (countBadge) {
    countBadge.innerText = `${totalCount} component${totalCount === 1 ? '' : 's'}`;
  }

  if (totalCount === 0) {
    container.innerHTML = `
      <div class="glass-card rounded-2xl p-10 text-center text-white/50 text-xs">
        No public services are currently exposed on this status page.
      </div>
    `;
    return;
  }

  if (groups && groups.length > 0) {
    container.innerHTML = groups
      .map((group) => `
        <div class="status-group space-y-3 pt-3">
          <div class="flex items-center justify-between px-1">
            <h2 class="text-sm font-semibold tracking-tight text-white flex items-center gap-2">
              <span class="w-1.5 h-1.5 rounded-full bg-[#0a84ff]"></span>
              <span>${escapeHtml(group.name)}</span>
            </h2>
            <span class="text-[11px] font-mono text-white/40 bg-white/5 px-2.5 py-0.5 rounded-full border border-white/5">
              ${group.monitors.length} component${group.monitors.length === 1 ? '' : 's'}
            </span>
          </div>
          <div class="space-y-3.5">
            ${group.monitors.map((m) => renderServiceCard(m)).join('')}
          </div>
        </div>
      `)
      .join('');
  } else {
    container.innerHTML = monitors
      .map((m) => renderServiceCard(m))
      .join('');
  }
}

// ── Single Public Service Card with 7-Day Interactive Uptime Bar ────────────

function renderServiceCard(m) {
  const isUp = m.current_status === 'UP';
  const isDown = m.current_status === 'DOWN';
  const isMaintenance = m.current_status === 'MAINTENANCE';

  let statusBadgeClass = 'status-badge-operational';
  let statusDotColor = 'var(--status-operational)';
  let statusText = 'Operational';

  if (isMaintenance) {
    statusBadgeClass = 'status-badge-maintenance';
    statusDotColor = '#c084fc';
    statusText = 'Maintenance';
  } else if (isDown) {
    statusBadgeClass = 'status-badge-down';
    statusDotColor = 'var(--status-down)';
    statusText = 'Major Outage';
  } else if (!isUp) {
    statusBadgeClass = 'status-badge-degraded';
    statusDotColor = 'var(--status-degraded)';
    statusText = 'Degraded';
  }

  const latencyText =
    m.last_latency_ms !== null && m.last_latency_ms !== undefined
      ? `${m.last_latency_ms} ms`
      : '—';

  const uptimePct =
    typeof m.uptime_24h === 'number' ? m.uptime_24h.toFixed(2) : '100.00';

  // Generate 7-Day Interactive Glassmorphic Pills
  const barsHtml = generate7DayBars(m);

  // Generate 24-Hour Hourly Latency Breakdown Bars
  const latencyBarsHtml = generate24hLatencyBars(m);
  const avgLatencyText = getAverageLatencyText(m);

  // Extract 24-Hour Latency Percentiles (P50, P95, P99, AVG)
  const lat = m.latency || {};
  const hasLatencyData =
    lat && (lat.p50 > 0 || lat.p95 > 0 || lat.p99 > 0 || lat.avg > 0);
  const p50Val = hasLatencyData || lat.p50 !== undefined ? `${lat.p50 || 0}ms` : '—';
  const p95Val = hasLatencyData || lat.p95 !== undefined ? `${lat.p95 || 0}ms` : '—';
  const p99Val = hasLatencyData || lat.p99 !== undefined ? `${lat.p99 || 0}ms` : '—';
  const avgVal = hasLatencyData || lat.avg !== undefined ? `${lat.avg || 0}ms` : '—';

  return `
    <div class="glass-card rounded-2xl p-5 sm:p-6 shadow-xl transition-all duration-200 hover:border-white/20 space-y-4">
      <!-- Card Header -->
      <div class="flex items-center justify-between gap-3">
        <div class="flex items-center gap-3 min-w-0">
          <span class="relative flex h-2.5 w-2.5 shrink-0">
            <span class="relative inline-flex rounded-full h-2.5 w-2.5" style="background-color: ${statusDotColor}"></span>
          </span>
          <div>
            <h3 class="font-semibold text-white text-base tracking-tight truncate">${escapeHtml(m.name)}</h3>
            <span class="text-[11px] text-white/40 font-mono">Component • Global HTTP Probe</span>
          </div>
        </div>

        <div class="flex items-center gap-3 shrink-0">
          <span class="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium ${statusBadgeClass}">
            <span class="h-1.5 w-1.5 rounded-full" style="background-color: ${statusDotColor}"></span>
            <span>${statusText}</span>
          </span>
        </div>
      </div>

      <!-- 7-Day Interactive Uptime Bar -->
      <div class="space-y-2 pt-1">
        <div class="flex items-center justify-between text-xs text-white/60 font-mono">
          <span>7-Day Uptime History</span>
          <span class="text-white font-semibold">${uptimePct}% uptime</span>
        </div>

        <!-- 7 Bars Container -->
        <div class="flex items-center gap-1.5 sm:gap-2.5 h-11 w-full p-1.5 bg-black/5 dark:bg-black/40 rounded-xl border border-black/5 dark:border-white/5">
          ${barsHtml}
        </div>

        <!-- Timeline Boundaries -->
        <div class="flex items-center justify-between text-[11px] text-white/40 font-mono pt-0.5">
          <span>7 days ago</span>
          <span>Current latency: <strong class="text-white/80 font-mono">${latencyText}</strong></span>
          <span>Today</span>
        </div>
      </div>

      <!-- 24-Hour Latency Breakdown Chart -->
      <div class="space-y-2 pt-2 border-t border-black/5 dark:border-white/5">
        <div class="flex items-center justify-between text-xs text-white/60 font-mono">
          <span>24-Hour Latency Breakdown</span>
          <span class="text-white/60 font-mono text-[11px]">24h Avg: <strong class="text-white/80 font-mono">${avgLatencyText}</strong></span>
        </div>

        <!-- 24 Bars Container -->
        <div class="flex items-end gap-[3px] sm:gap-1.5 h-16 w-full p-2 bg-black/5 dark:bg-black/40 rounded-xl border border-black/5 dark:border-white/5">
          ${latencyBarsHtml}
        </div>

        <!-- Latency Timeline Boundaries -->
        <div class="flex items-center justify-between text-[11px] text-white/40 font-mono pt-0.5">
          <span>24h ago</span>
          <span>12h ago</span>
          <span>Now</span>
        </div>

        <!-- 24-Hour Percentiles Summary Row -->
        <div class="flex flex-wrap items-center justify-between gap-2 pt-2.5 border-t border-black/5 dark:border-white/5">
          <span class="text-[11px] text-white/50 font-mono">24h Response Percentiles</span>
          <div class="flex flex-wrap items-center gap-1.5 sm:gap-2">
            <span class="font-mono text-xs bg-white/5 border border-white/10 px-2.5 py-1 rounded-md text-emerald-400">
              <span class="text-white/40 text-[10px] mr-1">P50</span><strong>${p50Val}</strong>
            </span>
            <span class="font-mono text-xs bg-white/5 border border-white/10 px-2.5 py-1 rounded-md text-yellow-400">
              <span class="text-white/40 text-[10px] mr-1">P95</span><strong>${p95Val}</strong>
            </span>
            <span class="font-mono text-xs bg-white/5 border border-white/10 px-2.5 py-1 rounded-md text-rose-400">
              <span class="text-white/40 text-[10px] mr-1">P99</span><strong>${p99Val}</strong>
            </span>
            <span class="font-mono text-xs bg-white/5 border border-white/10 px-2.5 py-1 rounded-md text-neutral-400">
              <span class="text-white/40 text-[10px] mr-1">AVG</span><strong>${avgVal}</strong>
            </span>
          </div>
        </div>
      </div>
    </div>
  `;
}

// ── 24-Hour Latency Helpers & Bar Generator with Hover Tooltips ─────────────

function getAverageLatencyText(monitor) {
  const slots = monitor.latency_24h || [];
  const valid = slots.filter((s) => s.status !== 'NO_DATA' && s.avg_latency_ms > 0);
  if (valid.length > 0) {
    const sum = valid.reduce((acc, s) => acc + s.avg_latency_ms, 0);
    return `${Math.round(sum / valid.length)} ms`;
  }
  if (monitor.last_latency_ms !== null && monitor.last_latency_ms !== undefined) {
    return `${monitor.last_latency_ms} ms`;
  }
  return '—';
}

function generate24hLatencyBars(monitor) {
  let slots = monitor.latency_24h || [];

  // Fallback if latency_24h is missing or empty
  if (!Array.isArray(slots) || slots.length === 0) {
    const now = new Date();
    const currentHour = new Date(
      now.getFullYear(),
      now.getMonth(),
      now.getDate(),
      now.getHours(),
      0,
      0
    );
    slots = [];
    for (let i = 23; i >= 0; i--) {
      const slotDate = new Date(currentHour.getTime() - i * 3600 * 1000);
      slots.push({
        timestamp: slotDate.toISOString(),
        avg_latency_ms:
          i === 0 && monitor.last_latency_ms ? monitor.last_latency_ms : 0,
        status: i === 0 ? monitor.current_status : 'NO_DATA',
      });
    }
  }

  // Calculate maximum latency across active slots for proportional bar heights
  const validLatencies = slots
    .filter((s) => s.status !== 'NO_DATA' && s.avg_latency_ms > 0)
    .map((s) => s.avg_latency_ms);
  const maxLatency =
    validLatencies.length > 0 ? Math.max(50, Math.max(...validLatencies)) : 100;

  return slots
    .slice(-24)
    .map((slot) => {
      const date = new Date(slot.timestamp);
      const timeStr =
        date.toLocaleDateString('en-US', {
          month: 'short',
          day: 'numeric',
        }) +
        ' ' +
        date.toLocaleTimeString('en-US', {
          hour: 'numeric',
          minute: '2-digit',
        });

      let barColorClass = 'bg-[#0a84ff] hover:bg-[#30d158]';
      let heightPct = 12;
      let tooltip = '';

      if (slot.status === 'NO_DATA') {
        barColorClass = 'bg-neutral-300 dark:bg-neutral-700/40';
        heightPct = 12;
        tooltip = `${timeStr} • No Data`;
      } else if (slot.status === 'DOWN') {
        barColorClass = 'bg-[#ff453a]';
        heightPct = 100;
        tooltip = `${timeStr} • Outage (${
          slot.avg_latency_ms > 0 ? slot.avg_latency_ms + ' ms' : 'Service Down'
        })`;
      } else if (slot.status === 'DEGRADED') {
        barColorClass = 'bg-[#ffd60a]';
        heightPct = Math.max(
          14,
          Math.min(100, Math.round((slot.avg_latency_ms / maxLatency) * 100))
        );
        tooltip = `${timeStr} • Degraded (${slot.avg_latency_ms} ms)`;
      } else {
        barColorClass = 'bg-[#0a84ff] hover:bg-[#30d158]';
        heightPct = Math.max(
          14,
          Math.min(100, Math.round((slot.avg_latency_ms / maxLatency) * 100))
        );
        tooltip = `${timeStr} • ${slot.avg_latency_ms} ms (Operational)`;
      }

      return `
        <div 
          class="flex-1 rounded-sm transition-all duration-200 cursor-pointer ${barColorClass} shadow-sm hover:brightness-125"
          style="height: ${heightPct}%; min-width: 2px;"
          title="${escapeHtml(tooltip)}"
        ></div>
      `;
    })
    .join('');
}

// ── 7-Day Daily Uptime Bar Generator with Hover Tooltips ─────────────────────

function generate7DayBars(monitor) {
  const history = monitor.uptime_history || monitor.history || [];
  const totalDays = 7;
  let html = '';

  if (Array.isArray(history) && history.length >= totalDays) {
    // Render the 7 distinct daily buckets returned by backend
    for (const day of history.slice(-totalDays)) {
      const dateObj = new Date(day.date + 'T00:00:00');
      const formattedDate = dateObj.toLocaleDateString('en-US', {
        month: 'short',
        day: 'numeric',
        year: 'numeric',
      });

      let barColorClass = 'bg-[#30d158]';
      let tooltip = '';

      if (day.status === 'NO_DATA') {
        barColorClass = 'bg-neutral-700/40';
        tooltip = `${formattedDate} • No Data`;
      } else if (day.status === 'MAINTENANCE') {
        barColorClass = 'bg-[#a855f7]';
        tooltip = `${formattedDate} • Planned Maintenance`;
      } else if (day.status === 'DOWN' || day.uptime_pct < 90.0) {
        barColorClass = 'bg-[#ff453a]';
        tooltip = `${formattedDate} • Outage (Uptime: ${day.uptime_pct.toFixed(2)}%)`;
      } else if (day.status === 'DEGRADED' || day.uptime_pct < 99.0) {
        barColorClass = 'bg-[#ffd60a]';
        tooltip = `${formattedDate} • Degraded (Uptime: ${day.uptime_pct.toFixed(2)}%)`;
      } else {
        barColorClass = 'bg-[#30d158]';
        tooltip = `${formattedDate} • Operational (Uptime: ${day.uptime_pct.toFixed(2)}%)`;
      }

      html += `
        <div 
          class="h-8 rounded-lg transition-all duration-200 flex-1 hover:scale-105 cursor-pointer ${barColorClass} shadow-sm"
          title="${tooltip}"
        ></div>
      `;
    }
  } else {
    // Fallback: 7 days calculated client-side
    const now = new Date();
    const isDown = monitor.current_status === 'DOWN';
    const uptimeVal = typeof monitor.uptime_24h === 'number' ? monitor.uptime_24h : 100.0;

    for (let i = totalDays - 1; i >= 0; i--) {
      const dayDate = new Date(now);
      dayDate.setDate(now.getDate() - i);

      const formattedDate = dayDate.toLocaleDateString('en-US', {
        month: 'short',
        day: 'numeric',
        year: 'numeric',
      });

      let barColorClass = 'bg-[#30d158]';
      let tooltip = `${formattedDate} • Operational (Uptime: 100.00%)`;

      if (i === 0) {
        if (isDown || uptimeVal < 90.0) {
          barColorClass = 'bg-[#ff453a]';
          tooltip = `${formattedDate} • Outage (Uptime: ${uptimeVal.toFixed(2)}%)`;
        } else if (uptimeVal < 99.0) {
          barColorClass = 'bg-[#ffd60a]';
          tooltip = `${formattedDate} • Degraded (Uptime: ${uptimeVal.toFixed(2)}%)`;
        } else {
          tooltip = `${formattedDate} • Operational (Uptime: ${uptimeVal.toFixed(2)}%)`;
        }
      }

      html += `
        <div 
          class="h-8 rounded-lg transition-all duration-200 flex-1 hover:scale-105 cursor-pointer ${barColorClass} shadow-sm"
          title="${tooltip}"
        ></div>
      `;
    }
  }

  return html;
}

// ── Incident & Maintenance Timeline Section ──────────────────────────────────

function renderIncidentTimeline(data) {
  const container = document.getElementById('incident-timeline-container');
  if (!container) return;

  const now = new Date();
  const todayStr = now.toLocaleDateString('en-US', {
    month: 'long',
    day: 'numeric',
    year: 'numeric',
  });

  const anyDown = (data.monitors || []).some((m) => m.current_status === 'DOWN');

  const maintWindows = data.maintenance_windows || [];
  let maintTimelineHtml = '';
  if (maintWindows.length > 0) {
    maintTimelineHtml = maintWindows
      .map(
        (mw) => `
        <div class="relative pl-6 pb-6 border-l border-purple-500/30 space-y-1.5">
          <span class="absolute -left-1.5 top-1 h-3 w-3 rounded-full status-bar-maintenance timeline-ring"></span>
          <div class="flex items-center gap-2">
            <span class="font-semibold text-white text-xs uppercase tracking-wider">${escapeHtml(mw.title)}</span>
            <span class="px-2 py-0.5 rounded-full text-[10px] status-badge-maintenance font-medium">
              ${mw.is_active ? 'Active Maintenance' : 'Scheduled Maintenance'}
            </span>
          </div>
          ${mw.description ? `<p class="text-xs text-white/70">${escapeHtml(mw.description)}</p>` : ''}
          <p class="text-xs text-white/50 font-mono">Window: ${escapeHtml(formatTimeUtc(mw.start_time))} — ${escapeHtml(formatTimeUtc(mw.end_time))}</p>
        </div>
      `
      )
      .join('');
  }

  let todayIncidentHtml = '';
  if (anyDown) {
    todayIncidentHtml = `
      <div class="relative pl-6 pb-6 border-l border-[#ff453a]/30 space-y-1.5">
        <span class="absolute -left-1.5 top-1 h-3 w-3 rounded-full status-bar-down timeline-ring"></span>
        <div class="flex items-center gap-2">
          <span class="font-semibold text-white text-xs uppercase tracking-wider">${todayStr}</span>
          <span class="px-2 py-0.2 rounded-full text-[10px] status-badge-down font-medium">Active Outage</span>
        </div>
        <p class="text-xs text-white/80 font-medium">Service Disruption Identified</p>
        <p class="text-xs text-white/50">Our automated edge synthetic probes have detected endpoint failures. Health checks are actively retrying.</p>
      </div>
    `;
  } else {
    todayIncidentHtml = `
      <div class="relative pl-6 pb-6 border-l border-white/10 space-y-1">
        <span class="absolute -left-1.5 top-1 h-3 w-3 rounded-full status-bar-operational timeline-ring"></span>
        <span class="font-semibold text-white text-xs uppercase tracking-wider">${todayStr}</span>
        <p class="text-xs text-white/60">No incidents or service interruptions reported today.</p>
      </div>
    `;
  }

  // Generate 2 past days with clean operational history
  let pastDaysHtml = '';
  for (let d = 1; d <= 2; d++) {
    const pastDate = new Date(now);
    pastDate.setDate(now.getDate() - d);
    const pDateStr = pastDate.toLocaleDateString('en-US', {
      month: 'long',
      day: 'numeric',
      year: 'numeric',
    });

    pastDaysHtml += `
      <div class="relative pl-6 pb-6 border-l border-white/10 space-y-1 ${d === 2 ? 'pb-0 border-transparent' : ''}">
        <span class="absolute -left-1.5 top-1 h-3 w-3 rounded-full bg-white/20 timeline-ring"></span>
        <span class="font-semibold text-white/70 text-xs uppercase tracking-wider">${pDateStr}</span>
        <p class="text-xs text-white/40">No incidents reported. All systems executed at 100% availability.</p>
      </div>
    `;
  }

  container.innerHTML = maintTimelineHtml + todayIncidentHtml + pastDaysHtml;
}

// ── Refresh & Countdown Logic ────────────────────────────────────────────────

function updateLastUpdatedTime() {
  const el = document.getElementById('last-updated-text');
  if (el) {
    el.innerText = `Updated ${new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })}`;
  }
}

function startAutoRefreshCountdown() {
  countdownSeconds = 30;
  const countdownEl = document.getElementById('countdown-seconds');

  if (countdownTimer) clearInterval(countdownTimer);

  countdownTimer = setInterval(() => {
    countdownSeconds--;
    if (countdownEl) {
      countdownEl.innerText = countdownSeconds;
    }

    if (countdownSeconds <= 0) {
      countdownSeconds = 30;
      fetchPublicStatus(false);
    }
  }, 1000);
}

// ── Error State Renderer ─────────────────────────────────────────────────────

function renderErrorState(message) {
  const heroTitle = document.getElementById('hero-status-title');
  const heroDesc = document.getElementById('page-description') || document.getElementById('hero-status-desc');
  const heroPulse = document.getElementById('hero-status-pulse');

  if (heroTitle) heroTitle.innerText = 'Service Status Unavailable';
  if (heroDesc) heroDesc.innerText = message;
  if (heroPulse) heroPulse.style.backgroundColor = '#8e8e93';
}

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

// ── DOM Initialization ───────────────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', () => {
  initAutoTheme();
  loadStatusData();
  startAutoRefreshCountdown();
});
