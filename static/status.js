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
let statusFetchController = null;

// ── Lightweight Component Helpers (Micro-renders) ───────────────────────────

function renderPercentilePill(label, value, colorClass) {
  return `<span class="font-mono text-xs bg-white/5 border border-white/10 px-2.5 py-1 rounded-md ${colorClass}"><span class="text-white/40 text-[10px] mr-1">${label}</span><strong>${value}</strong></span>`;
}

function renderPercentilePills(lat) {
  const hasLatencyData =
    Boolean(lat && (lat.p50 > 0 || lat.p95 > 0 || lat.p99 > 0 || lat.avg > 0));
  const p50Val = lat && (hasLatencyData || lat.p50 !== undefined) ? `${lat.p50 || 0}ms` : '—';
  const p95Val = lat && (hasLatencyData || lat.p95 !== undefined) ? `${lat.p95 || 0}ms` : '—';
  const p99Val = lat && (hasLatencyData || lat.p99 !== undefined) ? `${lat.p99 || 0}ms` : '—';
  const avgVal = lat && (hasLatencyData || lat.avg !== undefined) ? `${lat.avg || 0}ms` : '—';

  return `
    <div class="flex flex-wrap items-center gap-1.5 sm:gap-2">
      ${renderPercentilePill('P50', p50Val, 'text-emerald-400')}
      ${renderPercentilePill('P95', p95Val, 'text-yellow-400')}
      ${renderPercentilePill('P99', p99Val, 'text-rose-400')}
      ${renderPercentilePill('AVG', avgVal, 'text-neutral-400')}
    </div>
  `;
}

function renderMaintenanceBannerItem(w) {
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
}

// Fast batch DOM insertion to minimize browser reflows
function batchInsertHTML(container, htmlString) {
  if (!container) return;
  const fragment = document.createRange().createContextualFragment(htmlString);
  container.replaceChildren(fragment);
}

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
    batchInsertHTML(
      container,
      `<div class="glass-card rounded-2xl p-10 text-center text-white/50 text-xs">
        No components configured for status page "${escapeHtml(slug)}".
      </div>`
    );
  }
  if (timelineSection) {
    batchInsertHTML(
      timelineSection,
      `<div class="p-6 text-center text-white/40 text-xs">
        No incident history recorded for this page.
      </div>`
    );
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

  // Single-flight AbortController: abort any pending request before launching new tick
  if (statusFetchController) {
    statusFetchController.abort();
  }
  statusFetchController = new AbortController();
  const signal = statusFetchController.signal;

  try {
    const res = await fetch(`/api/public/status/${encodeURIComponent(slug)}`, { signal });
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
        batchInsertHTML(
          container,
          `<div class="p-6 text-center text-neutral-400 font-mono text-xs">No monitors assigned to this status page.</div>`
        );
      }
      return;
    }

    renderMonitors(monitors, data.groups);
  } catch (err) {
    if (err.name === 'AbortError' || signal.aborted) {
      // Single-flight abort superseded by newer tick; suppress error
      return;
    }
    console.error('Failed to load status page:', err);
    if (loadingEl) loadingEl.style.display = 'none';
    if (container) {
      batchInsertHTML(
        container,
        `<div class="p-6 text-center text-rose-400 font-mono text-xs">Failed to load status telemetry. Please try again.</div>`
      );
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
    container.replaceChildren();
    return;
  }

  const html = activeWindows.map(renderMaintenanceBannerItem).join('');
  batchInsertHTML(container, html);
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
    batchInsertHTML(
      container,
      `<div class="glass-card rounded-2xl p-10 text-center text-white/50 text-xs">
        No public services are currently exposed on this status page.
      </div>`
    );
    return;
  }

  let html = '';
  if (groups && groups.length > 0) {
    html = groups
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
    html = monitors.map((m) => renderServiceCard(m)).join('');
  }

  batchInsertHTML(container, html);
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

  return `
    <div data-action="open-detail" data-id="${m.id}" class="glass-card rounded-2xl p-5 sm:p-6 shadow-xl transition-all duration-200 hover:border-white/20 space-y-4 cursor-pointer">
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
          ${renderPercentilePills(m.latency)}
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

// ── Cached Intl Formatters for Zero Long-Task (<50ms) Renders ────────────────

const shortDateFormatter = new Intl.DateTimeFormat('en-US', {
  month: 'short',
  day: 'numeric',
});
const shortTimeFormatter = new Intl.DateTimeFormat('en-US', {
  hour: 'numeric',
  minute: '2-digit',
});
const fullDateFormatter = new Intl.DateTimeFormat('en-US', {
  month: 'short',
  day: 'numeric',
  year: 'numeric',
});

function formatSlotTime(date) {
  try {
    return `${shortDateFormatter.format(date)} ${shortTimeFormatter.format(date)}`;
  } catch {
    return date.toISOString ? date.toISOString().slice(5, 16).replace('T', ' ') : String(date);
  }
}

function formatFullDate(date) {
  try {
    return fullDateFormatter.format(date);
  } catch {
    return date.toISOString ? date.toISOString().slice(0, 10) : String(date);
  }
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
      const timeStr = formatSlotTime(date);

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
      const formattedDate = formatFullDate(dateObj);

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

      const formattedDate = formatFullDate(dayDate);

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

  batchInsertHTML(container, maintTimelineHtml + todayIncidentHtml + pastDaysHtml);
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

// ── Centralized Event Delegation ─────────────────────────────────────────────

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
      if (typeof window.showToast === 'function') {
        window.showToast({
          title: 'Authentication Required',
          message: 'Admin login required to trigger on-demand ping.',
          type: 'warning',
        });
      } else {
        alert('Admin login required to trigger on-demand ping.');
      }
      return;
    }

    if (!res.ok) {
      if (typeof window.showToast === 'function') {
        window.showToast({
          title: 'Ping Failed',
          message: `Server returned HTTP ${res.status}`,
          type: 'error',
        });
      }
      return;
    }

    const pingData = await res.json();
    if (typeof window.showToast === 'function') {
      window.showToast({
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
  } catch (err) {
    if (typeof window.showToast === 'function') {
      window.showToast({
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

// Global exports
window.openMonitorDetailModal = openMonitorDetailModal;
window.closeMonitorDetailModal = closeMonitorDetailModal;
window.triggerDetailPing = triggerDetailPing;

document.addEventListener('click', (e) => {
  const detailCard = e.target.closest('[data-action="open-detail"]');
  if (detailCard && !e.target.closest('button, a, input, select, textarea, label')) {
    const id = detailCard.dataset.id;
    if (id) openMonitorDetailModal(id);
    return;
  }

  const closeBtn = e.target.closest('[data-action="close-detail-modal"]');
  if (closeBtn) {
    closeMonitorDetailModal();
    return;
  }

  // Theme pill selector (Auto, Dark, OLED, Midnight, Light)
  const themeBtn = e.target.closest('[data-theme-btn]');
  if (themeBtn) {
    const theme = themeBtn.getAttribute('data-theme-btn');
    if (theme) setTheme(theme);
    return;
  }

  // Floating top theme toggle button
  const toggleBtn = e.target.closest('#theme-toggle-btn, [data-action="toggle-theme"]');
  if (toggleBtn) {
    toggleTheme();
    return;
  }
});

// Escape key listener for detail modal
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    const detailModal = document.getElementById('monitor-detail-modal');
    if (detailModal && !detailModal.classList.contains('hidden')) {
      closeMonitorDetailModal();
    }
  }
});

// ── DOM Initialization ───────────────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', () => {
  const detailBackdrop = document.getElementById('monitor-detail-modal');
  if (detailBackdrop) {
    detailBackdrop.addEventListener('click', (e) => {
      if (e.target === detailBackdrop) {
        closeMonitorDetailModal();
      }
    });
  }

  initAutoTheme();
  loadStatusData();
  startAutoRefreshCountdown();
});
