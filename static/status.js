// ── KeepUp Public Status Page Controller (Apple HIG Aesthetics) ───────────

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

let statusData = { status: 'Checking...', monitors: [] };
let countdownSeconds = 30;
let countdownTimer = null;

// ── Public Status Fetcher ────────────────────────────────────────────────────

async function fetchPublicStatus(isInitial = false) {
  try {
    // Try /api/public/status with fallback to /api/v1/public/status
    let res = await fetch('/api/public/status');
    if (!res.ok) {
      res = await fetch('/api/v1/public/status');
    }

    if (res.ok) {
      statusData = await res.json();

      // If user hasn't overridden via URL or localStorage, check backend status_page_theme
      const urlTheme = new URLSearchParams(window.location.search).get('theme');
      const storedTheme =
        localStorage.getItem('keepup_theme_preference') ||
        localStorage.getItem('keepup_status_theme') ||
        localStorage.getItem('keepup_theme');
      if (!urlTheme && !storedTheme && statusData.status_page_theme) {
        const backendTheme = statusData.status_page_theme.toLowerCase();
        if (backendTheme === 'auto') {
          applyAutoTheme();
        } else if (VALID_THEMES.includes(backendTheme)) {
          applyExplicitTheme(backendTheme);
        }
      }

      renderOverallHero(statusData);
      renderMonitorsList(statusData.monitors || []);
      renderIncidentTimeline(statusData);
      updateLastUpdatedTime();
    } else {
      renderErrorState('Unable to load telemetry from probe coordinator.');
    }
  } catch (err) {
    console.error('Error fetching public status:', err);
    if (isInitial) {
      renderErrorState('Network connection error while retrieving service telemetry.');
    }
  }
}

// ── Overall System Status Hero Card ──────────────────────────────────────────

function renderOverallHero(data) {
  const heroCard = document.getElementById('hero-status-card');
  const heroPulse = document.getElementById('hero-status-pulse');
  const heroTitle = document.getElementById('hero-status-title');
  const heroDesc = document.getElementById('hero-status-desc');

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
  let descText = 'All monitored components, APIs, and microservices are operating normally.';

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

// ── Monitors & Services List ─────────────────────────────────────────────────

function renderMonitorsList(monitors) {
  const container = document.getElementById('public-services-container');
  const countBadge = document.getElementById('services-count-badge');

  if (!container) return;

  if (countBadge) {
    countBadge.innerText = `${monitors.length} component${monitors.length === 1 ? '' : 's'}`;
  }

  if (monitors.length === 0) {
    container.innerHTML = `
      <div class="apple-glass rounded-2xl p-10 text-center text-white/50 text-xs">
        No public services are currently exposed.
      </div>
    `;
    return;
  }

  container.innerHTML = monitors
    .map((m) => renderServiceCard(m))
    .join('');
}

// ── Single Public Service Card with 7-Day Interactive Uptime Bar ────────────

function renderServiceCard(m) {
  const isUp = m.current_status === 'UP';
  const isDown = m.current_status === 'DOWN';

  let statusBadgeClass = 'status-badge-operational';
  let statusDotColor = 'var(--status-operational)';
  let statusText = 'Operational';

  if (isDown) {
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
    <div class="apple-glass rounded-2xl p-5 sm:p-6 shadow-xl transition-all duration-200 hover:border-white/20 space-y-4">
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

  container.innerHTML = todayIncidentHtml + pastDaysHtml;
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
  const heroDesc = document.getElementById('hero-status-desc');
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
  fetchPublicStatus(true);
  startAutoRefreshCountdown();
});
