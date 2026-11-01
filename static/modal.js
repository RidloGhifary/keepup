// ── Modal Controller for KeepUp (Clean Minimalist Aesthetics) ──────────────

let cachedWebhooks = [];

// Helper to resolve modal DOM elements
function getModalElements() {
  const backdrop = document.getElementById('modal-backdrop');
  const card = document.getElementById('modal-card');
  return { backdrop, card };
}

// ── Tab Switching ────────────────────────────────────────────────────────────

function switchModalTab(tab) {
  const btnMonitors = document.getElementById('tab-btn-monitors');
  const btnWebhooks = document.getElementById('tab-btn-webhooks');
  const formMonitor = document.getElementById('monitor-form');
  const formWebhook = document.getElementById('webhook-form');

  const activeClass =
    'flex-1 py-2 px-3 rounded-xl transition-all duration-200 bg-white dark:bg-white/10 text-neutral-900 dark:text-white shadow-sm font-semibold';
  const inactiveClass =
    'flex-1 py-2 px-3 rounded-xl transition-all duration-200 text-neutral-500 dark:text-white/50 hover:text-neutral-900 dark:hover:text-white';

  if (tab === 'monitors') {
    btnMonitors.className = activeClass;
    btnWebhooks.className = inactiveClass;
    formMonitor.classList.remove('hidden');
    formWebhook.classList.add('hidden');
  } else {
    btnWebhooks.className = activeClass;
    btnMonitors.className = inactiveClass;
    formWebhook.classList.remove('hidden');
    formMonitor.classList.add('hidden');
    fetchWebhooksForModal();
  }
}

// ── Show / Close Animations ──────────────────────────────────────────────────

function showModal() {
  const { backdrop, card } = getModalElements();
  if (!backdrop || !card) return;

  backdrop.classList.remove('hidden');
  // Trigger smooth scale & opacity transition
  requestAnimationFrame(() => {
    card.classList.remove('scale-95', 'opacity-0');
    card.classList.add('scale-100', 'opacity-100');
  });
}

function closeModal() {
  const { backdrop, card } = getModalElements();
  if (!backdrop || !card) return;

  card.classList.remove('scale-100', 'opacity-100');
  card.classList.add('scale-95', 'opacity-0');

  setTimeout(() => {
    backdrop.classList.add('hidden');
  }, 200);
}

// ── Open Modals with Dynamic Data ────────────────────────────────────────────

function openEditMonitorModal(data) {
  switchModalTab('monitors');

  let monitor = null;
  if (typeof data === 'string') {
    monitor = (window.allMonitors || []).find((m) => m.id === data);
  } else if (data && typeof data === 'object') {
    monitor = data;
  }

  const title = document.getElementById('modal-title');
  const subtitle = document.getElementById('modal-subtitle');
  const submitBtn = document.getElementById('save-monitor-btn');

  if (monitor) {
    title.innerText = 'Edit Health Monitor';
    subtitle.innerText = `Updating target configuration for "${monitor.name}"`;
    submitBtn.innerText = 'Save Changes';

    document.getElementById('edit-monitor-id').value = monitor.id;
    document.getElementById('m-name').value = monitor.name;
    document.getElementById('m-url').value = monitor.url;
    document.getElementById('m-interval').value = monitor.check_interval_secs;
    document.getElementById('m-timeout').value = monitor.timeout_secs;
    document.getElementById('m-retries').value = monitor.max_retries;
    document.getElementById('m-active').checked = monitor.is_active === 1;
    document.getElementById('m-public').checked = monitor.is_public === 1;
  } else {
    title.innerText = 'New Health Monitor';
    subtitle.innerText = 'Add a new service to ping and track uptime';
    submitBtn.innerText = 'Create Target';

    document.getElementById('edit-monitor-id').value = '';
    document.getElementById('monitor-form').reset();
    document.getElementById('m-interval').value = 60;
    document.getElementById('m-timeout').value = 5;
    document.getElementById('m-retries').value = 3;
    document.getElementById('m-active').checked = true;
    document.getElementById('m-public').checked = true;
  }

  showModal();
}

function openEditWebhookModal(data) {
  switchModalTab('webhooks');

  let webhook = null;
  if (typeof data === 'string') {
    webhook = cachedWebhooks.find((w) => w.id === data);
  } else if (data && typeof data === 'object') {
    webhook = data;
  }

  populateWebhookForm(webhook);
  showModal();
}

function toggleTemplateMode() {
  const isDefaultEl = document.getElementById('w-use-default');
  const previewSection = document.getElementById('default-template-preview');
  const customSection = document.getElementById('custom-template-section');

  if (!isDefaultEl || !previewSection || !customSection) return;

  const isDefault = isDefaultEl.checked;
  if (isDefault) {
    previewSection.classList.remove('hidden');
    customSection.classList.add('hidden');
    updateTemplatePreview();
  } else {
    previewSection.classList.add('hidden');
    customSection.classList.remove('hidden');
  }
}

function updateTemplatePreview() {
  const providerEl = document.getElementById('w-provider');
  const card = document.getElementById('template-preview-card');
  if (!providerEl || !card) return;

  const provider = (providerEl.value || 'discord').toLowerCase();

  if (provider === 'discord') {
    card.innerHTML = `
      <div class="border-l-4 border-[#30d158] bg-[#202225]/90 rounded-r-xl p-3 space-y-2 font-sans shadow-md">
        <div class="font-semibold text-white text-xs flex items-center gap-1.5">
          <span>🟢</span> <span>API Service is UP</span>
        </div>
        <div class="grid grid-cols-2 gap-2 text-[11px] text-neutral-300">
          <div><strong class="text-neutral-400 block text-[10px] uppercase font-mono">Service</strong> API Service</div>
          <div><strong class="text-neutral-400 block text-[10px] uppercase font-mono">Status</strong> UP</div>
          <div class="col-span-2"><strong class="text-neutral-400 block text-[10px] uppercase font-mono">Target URL</strong> https://api.example.com/health</div>
          <div><strong class="text-neutral-400 block text-[10px] uppercase font-mono">Latency</strong> 45 ms</div>
          <div><strong class="text-neutral-400 block text-[10px] uppercase font-mono">HTTP Status</strong> 200</div>
        </div>
        <div class="text-[10px] text-neutral-400 pt-1.5 border-t border-white/5 font-mono flex items-center justify-between">
          <span>KeepUp Uptime Monitor</span>
          <span>Today at 18:30</span>
        </div>
      </div>
    `;
  } else if (provider === 'slack') {
    card.innerHTML = `
      <div class="bg-[#1a1d21]/90 rounded-xl p-3 space-y-2.5 font-sans border border-slate-800 shadow-md">
        <div class="font-bold text-white text-xs flex items-center gap-1.5">
          <span>:white_check_mark:</span> <span>API Service is UP</span>
        </div>
        <div class="grid grid-cols-2 gap-2 text-[11px] text-neutral-300">
          <div><span class="font-bold text-neutral-400 block text-[10px] uppercase font-mono">Monitor</span> API Service</div>
          <div><span class="font-bold text-neutral-400 block text-[10px] uppercase font-mono">Status</span> UP</div>
          <div class="col-span-2"><span class="font-bold text-neutral-400 block text-[10px] uppercase font-mono">Target URL</span> https://api.example.com/health</div>
          <div><span class="font-bold text-neutral-400 block text-[10px] uppercase font-mono">Response Time</span> 45 ms</div>
        </div>
        <div class="text-[10px] text-neutral-400 pt-1.5 border-t border-white/5 italic">Sent by KeepUp uptime monitor</div>
      </div>
    `;
  } else {
    card.innerHTML = `
      <pre class="text-[11px] text-emerald-600 dark:text-emerald-400 overflow-x-auto leading-relaxed font-mono p-1">{
  "event": "monitor.status_change",
  "monitor_id": "mon_01a...",
  "monitor_name": "API Service",
  "target_url": "https://api.example.com/health",
  "status": "UP",
  "response_time_ms": 45,
  "http_status": 200,
  "timestamp": "2026-10-30T18:30:00Z"
}</pre>
    `;
  }
}

function populateWebhookForm(webhook) {
  const title = document.getElementById('modal-title');
  const subtitle = document.getElementById('modal-subtitle');
  const submitBtn = document.getElementById('save-webhook-btn');
  const testBtn = document.getElementById('test-webhook-btn');

  if (webhook) {
    if (title) title.innerText = 'Edit Webhook Channel';
    if (subtitle) subtitle.innerText = `Update alert channel for "${webhook.name}"`;
    if (submitBtn) submitBtn.innerText = 'Save Webhook';

    const idEl = document.getElementById('edit-webhook-id');
    const nameEl = document.getElementById('w-name');
    const providerEl = document.getElementById('w-provider');
    const urlEl = document.getElementById('w-url');
    const enabledEl = document.getElementById('w-enabled');
    const notifyResolveEl = document.getElementById('w-notify-resolve');
    const useDefaultEl = document.getElementById('w-use-default');
    const templateEl = document.getElementById('w-template');

    if (idEl) idEl.value = webhook.id;
    if (nameEl) nameEl.value = webhook.name;
    if (providerEl) providerEl.value = (webhook.provider || 'discord').toLowerCase();
    if (urlEl) urlEl.value = webhook.url;

    const isEnabled = webhook.is_enabled === 1 || webhook.is_enabled === true;
    if (enabledEl) enabledEl.checked = isEnabled;

    const notifyResolve =
      webhook.notify_on_resolve !== 0 &&
      webhook.notify_on_resolve !== false;
    if (notifyResolveEl) notifyResolveEl.checked = notifyResolve;

    const useDefault =
      webhook.use_default_template !== 0 &&
      webhook.use_default_template !== false;
    if (useDefaultEl) useDefaultEl.checked = useDefault;
    if (templateEl) templateEl.value = webhook.message_template || '';
  } else {
    if (title) title.innerText = 'Notifications & Webhooks';
    if (subtitle) subtitle.innerText = 'Dispatch instant alerts to Discord, Slack, or generic webhooks';
    if (submitBtn) submitBtn.innerText = 'Add Webhook';

    const idEl = document.getElementById('edit-webhook-id');
    const form = document.getElementById('webhook-form');
    const enabledEl = document.getElementById('w-enabled');
    const notifyResolveEl = document.getElementById('w-notify-resolve');
    const useDefaultEl = document.getElementById('w-use-default');
    const templateEl = document.getElementById('w-template');

    if (idEl) idEl.value = '';
    if (form) form.reset();
    if (enabledEl) enabledEl.checked = true;
    if (notifyResolveEl) notifyResolveEl.checked = true;
    if (useDefaultEl) useDefaultEl.checked = true;
    if (templateEl) templateEl.value = '';
  }

  // Ensure test notification button is always active and usable
  if (testBtn) {
    testBtn.disabled = false;
    testBtn.classList.remove('opacity-50', 'cursor-not-allowed');
  }

  toggleTemplateMode();
}

function resetWebhookForm() {
  populateWebhookForm(null);
}

// ── Form Submissions ─────────────────────────────────────────────────────────

async function handleMonitorSubmit(e) {
  if (e && typeof e.preventDefault === 'function') {
    e.preventDefault();
  }
  const id = document.getElementById('edit-monitor-id')?.value || '';
  const submitBtn = document.getElementById('save-monitor-btn');
  const originalText = id ? 'Save Changes' : 'Create Target';
  if (submitBtn) {
    submitBtn.disabled = true;
    submitBtn.innerText = id ? 'Saving...' : 'Creating...';
  }

  const payload = {
    name: document.getElementById('m-name')?.value?.trim() || '',
    url: document.getElementById('m-url')?.value?.trim() || '',
    check_interval_secs: parseInt(document.getElementById('m-interval')?.value || '60'),
    timeout_secs: parseInt(document.getElementById('m-timeout')?.value || '5'),
    max_retries: parseInt(document.getElementById('m-retries')?.value || '3'),
    is_active: document.getElementById('m-active') ? document.getElementById('m-active').checked : true,
    is_public: document.getElementById('m-public') ? document.getElementById('m-public').checked : true,
  };

  try {
    const url = id ? `/api/monitors/${id}` : '/api/v1/monitors';
    const method = id ? 'PUT' : 'POST';

    const res = await apiFetch(url, {
      method,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });

    if (res && res.ok) {
      closeModal();
      showToast({
        title: id ? 'Target Updated' : 'Target Created',
        message: id
          ? `Target "${payload.name}" updated (re-armed scheduler)`
          : `Target "${payload.name}" created`,
        type: 'success',
      });
      if (typeof fetchDashboardData === 'function') {
        fetchDashboardData();
      }
    } else if (res && res.status !== 401 && res.status !== 429) {
      const err = await res.json().catch(() => ({}));
      showToast({
        title: 'Save Failed',
        message: err.error || 'Failed to save monitor target',
        type: 'error',
      });
    }
  } catch (err) {
    // Already handled by apiFetch
  } finally {
    if (submitBtn) {
      submitBtn.disabled = false;
      submitBtn.innerText = originalText;
    }
  }
}

async function submitWebhookForm(e) {
  if (e && typeof e.preventDefault === 'function') {
    e.preventDefault();
  }

  const idEl = document.getElementById('edit-webhook-id');
  const id = idEl ? idEl.value.trim() : '';
  const submitBtn = document.getElementById('save-webhook-btn');
  const originalText = id ? 'Save Webhook' : 'Add Webhook';

  if (submitBtn) {
    submitBtn.disabled = true;
    submitBtn.innerText = id ? 'Saving...' : 'Adding...';
  }

  try {
    const name = (document.getElementById('w-name')?.value || '').trim();
    const provider = (document.getElementById('w-provider')?.value || 'discord').toLowerCase();
    const url = (document.getElementById('w-url')?.value || '').trim();
    const isEnabled = document.getElementById('w-enabled') ? document.getElementById('w-enabled').checked : true;
    const notifyResolve = document.getElementById('w-notify-resolve') ? document.getElementById('w-notify-resolve').checked : true;
    const useDefault = document.getElementById('w-use-default') ? document.getElementById('w-use-default').checked : true;
    const customTemplateVal = (document.getElementById('w-template')?.value || '').trim();

    if (!name || !url) {
      showToast({
        title: 'Validation Error',
        message: 'Name and endpoint URL are required.',
        type: 'warning',
      });
      return;
    }

    const payload = {
      name,
      provider,
      url,
      is_enabled: isEnabled,
      use_default_template: useDefault,
      notify_on_resolve: notifyResolve,
      message_template: useDefault ? null : (customTemplateVal || null),
    };

    const endpoint = id ? `/api/webhooks/${id}` : '/api/webhooks';
    const method = id ? 'PUT' : 'POST';

    const res = await apiFetch(endpoint, {
      method,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });

    if (res && (res.ok || res.status === 201)) {
      closeModal();
      showToast({
        title: id ? 'Webhook Updated' : 'Webhook Added',
        message: id
          ? `Webhook "${payload.name}" updated successfully`
          : `Webhook "${payload.name}" registered successfully`,
        type: 'success',
      });

      if (typeof fetchDashboardData === 'function') {
        fetchDashboardData();
      }
      if (typeof fetchWebhooksForModal === 'function') {
        fetchWebhooksForModal();
      }
    } else if (res && res.status !== 401 && res.status !== 429) {
      const err = await res.json().catch(() => ({}));
      showToast({
        title: 'Save Failed',
        message: err.error || 'Failed to save webhook.',
        type: 'error',
      });
    }
  } catch (err) {
    showToast({
      title: 'Submission Error',
      message: err.message || 'An error occurred while saving webhook.',
      type: 'error',
    });
  } finally {
    if (submitBtn) {
      submitBtn.disabled = false;
      submitBtn.innerText = originalText;
    }
  }
}

// ── Test Webhook Action ──────────────────────────────────────────────────────

async function testWebhook(e) {
  if (e && typeof e.preventDefault === 'function') {
    e.preventDefault();
  }

  const testBtn = document.getElementById('test-webhook-btn');
  let originalHtml = '';
  if (testBtn) {
    originalHtml = testBtn.innerHTML;
    testBtn.disabled = true;
    testBtn.innerHTML = `
      <svg class="w-4 h-4 animate-spin text-[#0a84ff]" fill="none" viewBox="0 0 24 24">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
      </svg>
      <span>Testing...</span>
    `;
  }

  try {
    const url = (document.getElementById('w-url')?.value || '').trim();
    const provider = (document.getElementById('w-provider')?.value || 'discord').toLowerCase();
    const useDefault = document.getElementById('w-use-default') ? document.getElementById('w-use-default').checked : true;
    const templateVal = (document.getElementById('w-template')?.value || '').trim();

    if (!url) {
      showToast({
        title: 'Endpoint Required',
        message: 'Enter a webhook endpoint URL before sending a test notification.',
        type: 'warning',
      });
      return;
    }

    const payload = {
      url,
      provider,
      use_default_template: useDefault,
      message_template: useDefault ? null : (templateVal || null),
    };

    const res = await apiFetch('/api/webhooks/test', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });

    if (res) {
      const data = await res.json().catch(() => ({}));
      if (res.ok) {
        showToast({
          title: 'Notification Delivered',
          message: data.message || 'Test notification delivered successfully.',
          type: 'success',
        });
      } else {
        showToast({
          title: 'Delivery Failed',
          message: data.error || 'Failed to send test payload to webhook.',
          type: 'warning',
        });
      }
    }
  } catch (err) {
    showToast({
      title: 'Test Failed',
      message: err.message || 'Network error while dispatching test payload.',
      type: 'error',
    });
  } finally {
    if (testBtn) {
      testBtn.disabled = false;
      testBtn.innerHTML = originalHtml;
    }
  }
}

// ── Webhook List in Modal ────────────────────────────────────────────────────

async function fetchWebhooksForModal() {
  const listContainer = document.getElementById('modal-webhooks-list');
  if (!listContainer) return;

  try {
    const res = await apiFetch('/api/webhooks');
    if (res && res.ok) {
      cachedWebhooks = await res.json();
      if (cachedWebhooks.length === 0) {
        listContainer.innerHTML = `
          <div class="text-[11px] text-neutral-500 dark:text-neutral-400 text-center py-2">
            No webhook channels configured yet. Fill out the form above to add one.
          </div>
        `;
        return;
      }

      listContainer.innerHTML = cachedWebhooks
        .map((w) => {
          const isEnabled = w.is_enabled === 1 || w.is_enabled === true;
          const providerBadge =
            w.provider.toLowerCase() === 'discord'
              ? 'bg-indigo-500/10 text-indigo-600 dark:text-indigo-400 border-indigo-500/20'
              : w.provider.toLowerCase() === 'slack'
              ? 'bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20'
              : 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/20';

          return `
          <div class="flex items-center justify-between p-2 rounded-xl bg-black/[0.03] dark:bg-black/40 border border-black/5 dark:border-white/10 hover:border-black/10 dark:hover:border-white/20 transition">
            <div class="min-w-0 pr-2">
              <div class="flex items-center gap-1.5">
                <span class="font-medium text-neutral-900 dark:text-white text-xs truncate">${escapeHtml(w.name)}</span>
                <span class="px-1.5 py-0.5 rounded text-[10px] uppercase font-mono border ${providerBadge}">${escapeHtml(w.provider)}</span>
                ${!isEnabled ? '<span class="text-[10px] text-rose-600 dark:text-rose-400 font-medium">(Disabled)</span>' : ''}
              </div>
              <div class="text-[10px] text-neutral-500 dark:text-neutral-400 truncate max-w-xs font-mono">${escapeHtml(w.url)}</div>
            </div>
            <div class="flex items-center gap-1 shrink-0">
              <button type="button" onclick="populateWebhookForm(cachedWebhooks.find(x => x.id === '${w.id}'))" class="text-xs text-neutral-600 hover:text-neutral-900 dark:text-neutral-400 dark:hover:text-white px-2 py-1 rounded hover:bg-black/5 dark:hover:bg-white/10 transition">Edit</button>
              <button type="button" onclick="deleteWebhook('${w.id}', '${escapeHtml(w.name)}')" class="text-xs text-rose-600 hover:text-rose-700 dark:text-rose-400 dark:hover:text-rose-300 px-2 py-1 rounded hover:bg-rose-500/10 transition">Delete</button>
            </div>
          </div>
        `;
        })
        .join('');
    }
  } catch (err) {
    console.error('Error fetching webhooks:', err);
  }
}

async function deleteWebhook(id, name) {
  if (!confirm(`Delete alert webhook "${name}"?`)) return;

  try {
    const res = await apiFetch(`/api/webhooks/${id}`, { method: 'DELETE' });
    if (res && res.ok) {
      showToast({
        title: 'Webhook Removed',
        message: `Webhook "${name}" deleted successfully`,
        type: 'success',
      });
      if (document.getElementById('edit-webhook-id').value === id) {
        resetWebhookForm();
      }
      fetchWebhooksForModal();
    } else if (res && res.status !== 401 && res.status !== 429) {
      showToast({
        title: 'Delete Failed',
        message: 'Failed to delete webhook',
        type: 'error',
      });
    }
  } catch (err) {
    // Already handled by apiFetch
  }
}

// ── Global Listeners for Modal (Backdrop & Escape Key) ───────────────────────

// ── Multi-Status Pages Controller ──────────────────────────────────────────

let cachedStatusPages = [];

async function openStatusPagesModal() {
  const backdrop = document.getElementById('status-pages-modal-backdrop');
  const card = document.getElementById('status-pages-modal-card');
  if (!backdrop || !card) return;

  switchStatusPageTab('list');
  backdrop.classList.remove('hidden');
  requestAnimationFrame(() => {
    card.classList.remove('scale-95', 'opacity-0');
    card.classList.add('scale-100', 'opacity-100');
  });

  await fetchStatusPagesList();
}

function closeStatusPagesModal() {
  const backdrop = document.getElementById('status-pages-modal-backdrop');
  const card = document.getElementById('status-pages-modal-card');
  if (!backdrop || !card) return;

  card.classList.remove('scale-100', 'opacity-100');
  card.classList.add('scale-95', 'opacity-0');
  setTimeout(() => {
    backdrop.classList.add('hidden');
  }, 200);
}

function switchStatusPageTab(tab) {
  const tabList = document.getElementById('sp-tab-list');
  const tabForm = document.getElementById('sp-tab-form');
  const viewList = document.getElementById('sp-view-list');
  const viewForm = document.getElementById('sp-edit-form');

  if (!tabList || !tabForm || !viewList || !viewForm) return;

  if (tab === 'list') {
    tabList.className = 'flex-1 py-1.5 px-3 rounded-xl transition-all duration-200 bg-white/10 text-white font-semibold shadow-sm';
    tabForm.className = 'flex-1 py-1.5 px-3 rounded-xl transition-all duration-200 text-white/50 hover:text-white';
    viewList.classList.remove('hidden');
    viewForm.classList.add('hidden');
  } else {
    tabForm.className = 'flex-1 py-1.5 px-3 rounded-xl transition-all duration-200 bg-white/10 text-white font-semibold shadow-sm';
    tabList.className = 'flex-1 py-1.5 px-3 rounded-xl transition-all duration-200 text-white/50 hover:text-white';
    viewForm.classList.remove('hidden');
    viewList.classList.add('hidden');
  }
}

async function fetchStatusPagesList() {
  const container = document.getElementById('sp-pages-table-container');
  if (!container) return;

  try {
    const res = await (window.apiFetch || fetch)('/api/admin/status-pages');
    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      container.innerHTML = `<div class="p-6 text-center text-[#ff453a] text-xs">${escapeHtml(err.error || 'Failed to load status pages')}</div>`;
      return;
    }

    cachedStatusPages = await res.json();
    renderStatusPagesList(cachedStatusPages);
  } catch (err) {
    container.innerHTML = `<div class="p-6 text-center text-[#ff453a] text-xs">Network error loading status pages</div>`;
  }
}

function renderStatusPagesList(pages) {
  const container = document.getElementById('sp-pages-table-container');
  if (!container) return;

  if (pages.length === 0) {
    container.innerHTML = `
      <div class="p-8 text-center text-white/40 text-xs glass-card rounded-2xl">
        No status pages found.
      </div>
    `;
    return;
  }

  container.innerHTML = pages
    .map((p) => {
      const isDefault = p.slug === 'default';
      const pageUrl = isDefault ? '/status' : `/status/${encodeURIComponent(p.slug)}`;
      return `
        <div class="flex items-center justify-between p-3.5 rounded-2xl bg-white/5 border border-white/5 hover:border-white/10 transition">
          <div class="min-w-0 flex-1 pr-3">
            <div class="flex items-center gap-2">
              <span class="font-semibold text-white text-sm truncate">${escapeHtml(p.title)}</span>
              ${isDefault ? '<span class="text-[9px] font-mono uppercase bg-[#30d158]/20 text-[#30d158] px-2 py-0.5 rounded-full border border-[#30d158]/30">Default</span>' : ''}
              ${p.is_public === 0 ? '<span class="text-[9px] font-mono uppercase bg-white/10 text-white/50 px-1.5 py-0.5 rounded-md">Private</span>' : ''}
            </div>
            <div class="flex items-center gap-2 text-xs text-white/50 mt-1 font-mono">
              <a href="${pageUrl}" target="_blank" class="hover:text-[#0a84ff] hover:underline flex items-center gap-1">
                <span>/status/${escapeHtml(p.slug)}</span>
                <svg class="w-3 h-3 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"/></svg>
              </a>
              <span>•</span>
              <span>${p.monitor_count} monitor${p.monitor_count === 1 ? '' : 's'}</span>
            </div>
            ${p.description ? `<p class="text-[11px] text-white/40 truncate mt-1">${escapeHtml(p.description)}</p>` : ''}
          </div>
          <div class="flex items-center gap-1.5 shrink-0">
            <button type="button" onclick="openEditStatusPageForm('${escapeHtml(p.slug)}')" class="px-2.5 py-1.5 rounded-lg bg-white/5 hover:bg-white/10 text-white/80 hover:text-white text-xs font-medium transition" title="Edit status page and monitor assignments">
              Edit
            </button>
            ${!isDefault ? `
              <button type="button" onclick="deleteStatusPageConfirm('${escapeHtml(p.slug)}')" class="px-2.5 py-1.5 rounded-lg bg-white/5 hover:bg-[#ff453a]/20 text-white/50 hover:text-[#ff453a] text-xs font-medium transition" title="Delete status page">
                Delete
              </button>
            ` : ''}
          </div>
        </div>
      `;
    })
    .join('');
}

function openCreateStatusPageForm() {
  const isEditEl = document.getElementById('sp-is-edit');
  const slugEl = document.getElementById('sp-slug');
  const titleEl = document.getElementById('sp-title');
  const descEl = document.getElementById('sp-desc');
  const isPublicEl = document.getElementById('sp-is-public');
  const submitBtn = document.getElementById('sp-submit-btn');

  if (isEditEl) isEditEl.value = 'false';
  if (slugEl) {
    slugEl.value = '';
    slugEl.disabled = false;
  }
  if (titleEl) titleEl.value = '';
  if (descEl) descEl.value = '';
  if (isPublicEl) isPublicEl.checked = true;
  if (submitBtn) submitBtn.innerText = 'Create Status Page';

  populateMonitorsChecklist([]);
  switchStatusPageTab('form');
}

async function openEditStatusPageForm(slug) {
  const isEditEl = document.getElementById('sp-is-edit');
  const slugEl = document.getElementById('sp-slug');
  const titleEl = document.getElementById('sp-title');
  const descEl = document.getElementById('sp-desc');
  const isPublicEl = document.getElementById('sp-is-public');
  const submitBtn = document.getElementById('sp-submit-btn');

  try {
    const res = await (window.apiFetch || fetch)(`/api/admin/status-pages/${encodeURIComponent(slug)}`);
    if (!res.ok) {
      if (window.showToast) window.showToast('Failed to load status page details', 'error');
      return;
    }
    const page = await res.json();

    if (isEditEl) isEditEl.value = 'true';
    if (slugEl) {
      slugEl.value = page.slug;
      slugEl.disabled = true;
    }
    if (titleEl) titleEl.value = page.title;
    if (descEl) descEl.value = page.description || '';
    if (isPublicEl) isPublicEl.checked = page.is_public !== false;
    if (submitBtn) submitBtn.innerText = 'Save Changes';

    populateMonitorsChecklist(page.monitors || []);
    switchStatusPageTab('form');
  } catch (err) {
    if (window.showToast) window.showToast('Network error loading status page', 'error');
  }
}

function populateMonitorsChecklist(assignments = []) {
  const container = document.getElementById('sp-monitors-checklist');
  if (!container) return;

  const monitors = window.allMonitors || [];
  if (monitors.length === 0) {
    container.innerHTML = `
      <div class="text-center py-4 text-white/40 text-xs">
        No monitors configured in the system. Create monitors first before assigning.
      </div>
    `;
    return;
  }

  const assignmentMap = new Map();
  assignments.forEach((a) => {
    assignmentMap.set(a.monitor_id, a);
  });

  container.innerHTML = monitors
    .map((m) => {
      const assigned = assignmentMap.get(m.id);
      const isChecked = !!assigned;
      const groupName = assigned ? assigned.group_name : 'Services';
      const displayOrder = assigned ? assigned.display_order : 0;

      return `
        <div class="flex items-center justify-between gap-3 p-2 rounded-xl bg-white/5 border border-white/5 hover:border-white/10 transition text-xs">
          <div class="flex items-center gap-2.5 min-w-0 flex-1">
            <input type="checkbox" id="sp-m-${m.id}" data-monitor-id="${m.id}" ${isChecked ? 'checked' : ''} class="sp-m-check rounded border-white/20 bg-black/40 text-[#0a84ff] focus:ring-0 cursor-pointer">
            <label for="sp-m-${m.id}" class="truncate cursor-pointer flex-1">
              <div class="font-medium text-white truncate">${escapeHtml(m.name)}</div>
              <div class="text-[10px] text-white/40 font-mono truncate">${escapeHtml(m.url)}</div>
            </label>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <input type="text" id="sp-grp-${m.id}" value="${escapeHtml(groupName)}" placeholder="Group Name" title="Monitor Group" class="w-32 px-2.5 py-1 rounded-lg bg-black/50 border border-white/10 text-white text-[11px] focus:outline-none focus:ring-1 focus:ring-[#0a84ff]">
            <input type="number" id="sp-ord-${m.id}" value="${displayOrder}" placeholder="0" title="Display Order" class="w-12 px-1 py-1 rounded-lg bg-black/50 border border-white/10 text-white text-[11px] font-mono text-center focus:outline-none focus:ring-1 focus:ring-[#0a84ff]">
          </div>
        </div>
      `;
    })
    .join('');
}

async function handleStatusPageSubmit(event) {
  event.preventDefault();

  const isEdit = document.getElementById('sp-is-edit').value === 'true';
  const slug = document.getElementById('sp-slug').value.trim().toLowerCase();
  const title = document.getElementById('sp-title').value.trim();
  const description = document.getElementById('sp-desc').value.trim() || null;
  const isPublic = document.getElementById('sp-is-public').checked;

  if (!slug) {
    if (window.showToast) window.showToast('Please enter a valid page slug', 'warning');
    return;
  }
  if (!title) {
    if (window.showToast) window.showToast('Please enter a title', 'warning');
    return;
  }

  // Gather assigned monitors
  const monitorChecks = document.querySelectorAll('.sp-m-check');
  const monitors = [];
  monitorChecks.forEach((chk) => {
    if (chk.checked) {
      const monitorId = chk.getAttribute('data-monitor-id');
      const grpInput = document.getElementById(`sp-grp-${monitorId}`);
      const ordInput = document.getElementById(`sp-ord-${monitorId}`);
      monitors.push({
        monitor_id: monitorId,
        group_name: grpInput && grpInput.value.trim() ? grpInput.value.trim() : 'Services',
        display_order: ordInput && ordInput.value ? parseInt(ordInput.value, 10) : 0,
      });
    }
  });

  const submitBtn = document.getElementById('sp-submit-btn');
  if (submitBtn) {
    submitBtn.disabled = true;
    submitBtn.innerText = 'Saving...';
  }

  try {
    const apiFetch = window.apiFetch || fetch;

    if (isEdit) {
      const res = await apiFetch(`/api/admin/status-pages/${encodeURIComponent(slug)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          title,
          description,
          is_public: isPublic,
          monitors,
        }),
      });

      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.error || 'Failed to update status page');
      }
    } else {
      // 1. Create status page
      const createRes = await apiFetch('/api/admin/status-pages', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          slug,
          title,
          description,
          is_public: isPublic,
          monitors,
        }),
      });

      if (!createRes.ok) {
        const err = await createRes.json().catch(() => ({}));
        throw new Error(err.error || 'Failed to create status page');
      }
    }

    if (window.showToast) {
      window.showToast(`Status page "${slug}" saved successfully`, 'success');
    }

    await fetchStatusPagesList();
    switchStatusPageTab('list');
  } catch (err) {
    if (window.showToast) {
      window.showToast(err.message || 'Error saving status page', 'error');
    }
  } finally {
    if (submitBtn) {
      submitBtn.disabled = false;
      submitBtn.innerText = isEdit ? 'Save Changes' : 'Create Status Page';
    }
  }
}

async function deleteStatusPageConfirm(slug) {
  if (slug === 'default') {
    if (window.showToast) window.showToast('Cannot delete default status page', 'error');
    return;
  }

  if (!confirm(`Are you sure you want to delete status page "${slug}"?`)) {
    return;
  }

  try {
    const apiFetch = window.apiFetch || fetch;
    const res = await apiFetch(`/api/admin/status-pages/${encodeURIComponent(slug)}`, {
      method: 'DELETE',
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || 'Failed to delete status page');
    }

    if (window.showToast) {
      window.showToast(`Status page "${slug}" deleted`, 'info');
    }
    await fetchStatusPagesList();
  } catch (err) {
    if (window.showToast) {
      window.showToast(err.message || 'Error deleting status page', 'error');
    }
  }
}

// ── Planned Maintenance Windows Controller ────────────────────────────────────

let cachedMaintenanceWindows = [];

async function openMaintenanceModal() {
  const backdrop = document.getElementById('maintenance-modal-backdrop');
  const card = document.getElementById('maintenance-modal-card');
  if (!backdrop || !card) return;

  switchMaintenanceTab('list');
  backdrop.classList.remove('hidden');
  requestAnimationFrame(() => {
    card.classList.remove('scale-95', 'opacity-0');
    card.classList.add('scale-100', 'opacity-100');
  });

  await fetchMaintenanceList();
}

function closeMaintenanceModal() {
  const backdrop = document.getElementById('maintenance-modal-backdrop');
  const card = document.getElementById('maintenance-modal-card');
  if (!backdrop || !card) return;

  card.classList.remove('scale-100', 'opacity-100');
  card.classList.add('scale-95', 'opacity-0');
  setTimeout(() => {
    backdrop.classList.add('hidden');
  }, 200);
}

function switchMaintenanceTab(tab) {
  const tabList = document.getElementById('maint-tab-list');
  const tabForm = document.getElementById('maint-tab-form');
  const viewList = document.getElementById('maint-view-list');
  const viewForm = document.getElementById('maint-edit-form');

  if (!tabList || !tabForm || !viewList || !viewForm) return;

  if (tab === 'list') {
    tabList.className = 'flex-1 py-1.5 px-3 rounded-xl transition-all duration-200 bg-white/10 text-white font-semibold shadow-sm';
    tabForm.className = 'flex-1 py-1.5 px-3 rounded-xl transition-all duration-200 text-white/50 hover:text-white';
    viewList.classList.remove('hidden');
    viewForm.classList.add('hidden');
  } else {
    tabForm.className = 'flex-1 py-1.5 px-3 rounded-xl transition-all duration-200 bg-white/10 text-white font-semibold shadow-sm';
    tabList.className = 'flex-1 py-1.5 px-3 rounded-xl transition-all duration-200 text-white/50 hover:text-white';
    viewForm.classList.remove('hidden');
    viewList.classList.add('hidden');
  }
}

async function fetchMaintenanceList() {
  const container = document.getElementById('maint-windows-table-container');
  if (!container) return;

  try {
    const apiFetch = window.apiFetch || fetch;
    const res = await apiFetch('/api/admin/maintenance');
    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      container.innerHTML = `<div class="p-6 text-center text-[#ff453a] text-xs">${escapeHtml(err.error || 'Failed to load maintenance windows')}</div>`;
      return;
    }

    cachedMaintenanceWindows = await res.json();
    renderMaintenanceList(cachedMaintenanceWindows);
  } catch (err) {
    container.innerHTML = `<div class="p-6 text-center text-[#ff453a] text-xs">Network error loading maintenance windows</div>`;
  }
}

function toLocalDatetimeInput(isoStr) {
  if (!isoStr) return '';
  const d = new Date(isoStr);
  const pad = (n) => String(n).padStart(2, '0');
  const year = d.getFullYear();
  const month = pad(d.getMonth() + 1);
  const day = pad(d.getDate());
  const hours = pad(d.getHours());
  const minutes = pad(d.getMinutes());
  return `${year}-${month}-${day}T${hours}:${minutes}`;
}

function renderMaintenanceList(windows) {
  const container = document.getElementById('maint-windows-table-container');
  if (!container) return;

  if (!windows || windows.length === 0) {
    container.innerHTML = `
      <div class="p-8 text-center text-white/40 text-xs glass-card rounded-2xl">
        No scheduled maintenance windows found.
      </div>
    `;
    return;
  }

  const now = new Date();

  container.innerHTML = windows
    .map((w) => {
      const start = new Date(w.start_time);
      const end = new Date(w.end_time);
      const isActive = w.is_active || (now >= start && now <= end);
      const isPast = now > end;

      let badgeHtml = '';
      if (isActive) {
        badgeHtml = '<span class="text-[9px] font-mono uppercase bg-purple-500/20 text-[#c084fc] px-2 py-0.5 rounded-full border border-purple-500/30 font-semibold">Active</span>';
      } else if (isPast) {
        badgeHtml = '<span class="text-[9px] font-mono uppercase bg-white/10 text-white/40 px-2 py-0.5 rounded-full border border-white/5">Completed</span>';
      } else {
        badgeHtml = '<span class="text-[9px] font-mono uppercase bg-amber-500/20 text-[#ffd60a] px-2 py-0.5 rounded-full border border-amber-500/30">Upcoming</span>';
      }

      const monitorCount = (w.monitor_ids || []).length;
      const monitorLabel = monitorCount === 0 ? 'All Monitors' : `${monitorCount} monitor${monitorCount === 1 ? '' : 's'}`;

      return `
        <div class="flex items-center justify-between p-3.5 rounded-2xl bg-white/5 border border-white/5 hover:border-white/10 transition">
          <div class="min-w-0 flex-1 pr-3">
            <div class="flex items-center gap-2">
              <span class="font-semibold text-white text-sm truncate">${escapeHtml(w.title)}</span>
              ${badgeHtml}
            </div>
            ${w.description ? `<p class="text-xs text-white/50 truncate mt-0.5">${escapeHtml(w.description)}</p>` : ''}
            <div class="flex items-center gap-2 text-[10px] text-white/40 font-mono mt-1">
              <span>${start.toLocaleString()} → ${end.toLocaleString()}</span>
              <span>•</span>
              <span class="text-white/60">${monitorLabel}</span>
            </div>
          </div>
          <div class="flex items-center gap-1.5 shrink-0">
            <button type="button" onclick="openEditMaintenanceForm('${w.id}')" class="p-1.5 rounded-lg bg-white/5 hover:bg-white/10 text-white/70 hover:text-white transition" title="Edit Maintenance Window">
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/></svg>
            </button>
            <button type="button" onclick="deleteMaintenanceConfirm('${w.id}', '${escapeHtml(w.title.replace(/'/g, "\\'"))}')" class="p-1.5 rounded-lg bg-white/5 hover:bg-[#ff453a]/20 text-white/70 hover:text-[#ff453a] transition" title="Delete Maintenance Window">
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
            </button>
          </div>
        </div>
      `;
    })
    .join('');
}

function openCreateMaintenanceForm() {
  const isEditEl = document.getElementById('maint-is-edit');
  const editIdEl = document.getElementById('maint-edit-id');
  const titleEl = document.getElementById('maint-title');
  const descEl = document.getElementById('maint-desc');
  const startEl = document.getElementById('maint-start-time');
  const endEl = document.getElementById('maint-end-time');
  const submitBtn = document.getElementById('maint-submit-btn');

  if (isEditEl) isEditEl.value = 'false';
  if (editIdEl) editIdEl.value = '';
  if (titleEl) titleEl.value = '';
  if (descEl) descEl.value = '';

  const now = new Date();
  const nextHour = new Date(now.getTime() + 60 * 60 * 1000);
  const twoHours = new Date(now.getTime() + 2 * 60 * 60 * 1000);
  if (startEl) startEl.value = toLocalDatetimeInput(nextHour);
  if (endEl) endEl.value = toLocalDatetimeInput(twoHours);
  if (submitBtn) submitBtn.innerText = 'Schedule Maintenance';

  populateMaintenanceMonitorsChecklist([]);
  switchMaintenanceTab('form');
}

function openEditMaintenanceForm(id) {
  const w = cachedMaintenanceWindows.find((item) => item.id === id);
  if (!w) return;

  const isEditEl = document.getElementById('maint-is-edit');
  const editIdEl = document.getElementById('maint-edit-id');
  const titleEl = document.getElementById('maint-title');
  const descEl = document.getElementById('maint-desc');
  const startEl = document.getElementById('maint-start-time');
  const endEl = document.getElementById('maint-end-time');
  const submitBtn = document.getElementById('maint-submit-btn');

  if (isEditEl) isEditEl.value = 'true';
  if (editIdEl) editIdEl.value = id;
  if (titleEl) titleEl.value = w.title;
  if (descEl) descEl.value = w.description || '';
  if (startEl) startEl.value = toLocalDatetimeInput(w.start_time);
  if (endEl) endEl.value = toLocalDatetimeInput(w.end_time);
  if (submitBtn) submitBtn.innerText = 'Save Changes';

  populateMaintenanceMonitorsChecklist(w.monitor_ids || []);
  switchMaintenanceTab('form');
}

function populateMaintenanceMonitorsChecklist(selectedIds = []) {
  const container = document.getElementById('maint-monitors-checklist');
  if (!container) return;

  const monitors = window.allMonitors || [];
  if (monitors.length === 0) {
    container.innerHTML = `
      <div class="text-center py-4 text-white/40 text-xs">
        No monitors configured. All monitors will be covered by default.
      </div>
    `;
    return;
  }

  const selectedSet = new Set(selectedIds);

  container.innerHTML = monitors
    .map((m) => {
      const isChecked = selectedSet.has(m.id);
      return `
        <div class="flex items-center justify-between gap-3 p-2 rounded-xl bg-white/5 border border-white/5 hover:border-white/10 transition text-xs">
          <div class="flex items-center gap-2.5 min-w-0 flex-1">
            <input type="checkbox" id="maint-m-${m.id}" data-monitor-id="${m.id}" ${isChecked ? 'checked' : ''} class="maint-m-check rounded border-white/20 bg-black/40 text-[#c084fc] focus:ring-0 cursor-pointer">
            <label for="maint-m-${m.id}" class="truncate cursor-pointer flex-1">
              <div class="font-medium text-white truncate">${escapeHtml(m.name)}</div>
              <div class="text-[10px] text-white/40 font-mono truncate">${escapeHtml(m.url)}</div>
            </label>
          </div>
        </div>
      `;
    })
    .join('');
}

function toggleAllMaintenanceMonitors() {
  const checks = document.querySelectorAll('.maint-m-check');
  const btn = document.getElementById('maint-toggle-all-btn');
  if (checks.length === 0) return;

  const anyUnchecked = Array.from(checks).some((c) => !c.checked);
  checks.forEach((c) => (c.checked = anyUnchecked));
  if (btn) {
    btn.innerText = anyUnchecked ? 'Deselect All' : 'Select All';
  }
}

async function handleMaintenanceSubmit(event) {
  event.preventDefault();

  const isEdit = document.getElementById('maint-is-edit').value === 'true';
  const id = document.getElementById('maint-edit-id').value;
  const title = document.getElementById('maint-title').value.trim();
  const description = document.getElementById('maint-desc').value.trim() || null;
  const startTimeInput = document.getElementById('maint-start-time').value;
  const endTimeInput = document.getElementById('maint-end-time').value;

  if (!title) {
    if (window.showToast) window.showToast('Please enter a maintenance title', 'warning');
    return;
  }
  if (!startTimeInput || !endTimeInput) {
    if (window.showToast) window.showToast('Please select start and end times', 'warning');
    return;
  }

  const startDt = new Date(startTimeInput);
  const endDt = new Date(endTimeInput);
  if (endDt <= startDt) {
    if (window.showToast) window.showToast('End time must be after start time', 'warning');
    return;
  }

  const monitorChecks = document.querySelectorAll('.maint-m-check');
  const monitor_ids = [];
  monitorChecks.forEach((chk) => {
    if (chk.checked) {
      monitor_ids.push(chk.getAttribute('data-monitor-id'));
    }
  });

  const submitBtn = document.getElementById('maint-submit-btn');
  if (submitBtn) {
    submitBtn.disabled = true;
    submitBtn.innerText = 'Saving...';
  }

  try {
    const apiFetch = window.apiFetch || fetch;
    const payload = {
      title,
      description,
      start_time: startDt.toISOString(),
      end_time: endDt.toISOString(),
      monitor_ids,
    };

    let res;
    if (isEdit) {
      res = await apiFetch(`/api/admin/maintenance/${encodeURIComponent(id)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
    } else {
      res = await apiFetch('/api/admin/maintenance', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
    }

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || 'Failed to save maintenance window');
    }

    if (window.showToast) {
      window.showToast(isEdit ? 'Maintenance window updated' : 'Maintenance window scheduled', 'success');
    }

    switchMaintenanceTab('list');
    await fetchMaintenanceList();
  } catch (err) {
    if (window.showToast) {
      window.showToast(err.message || 'Error saving maintenance window', 'error');
    }
  } finally {
    if (submitBtn) {
      submitBtn.disabled = false;
      submitBtn.innerText = isEdit ? 'Save Changes' : 'Schedule Maintenance';
    }
  }
}

async function deleteMaintenanceConfirm(id, title) {
  if (!confirm(`Are you sure you want to cancel and delete maintenance window "${title}"?`)) {
    return;
  }

  try {
    const apiFetch = window.apiFetch || fetch;
    const res = await apiFetch(`/api/admin/maintenance/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || 'Failed to delete maintenance window');
    }

    if (window.showToast) {
      window.showToast(`Maintenance window "${title}" deleted`, 'info');
    }
    await fetchMaintenanceList();
  } catch (err) {
    if (window.showToast) {
      window.showToast(err.message || 'Error deleting maintenance window', 'error');
    }
  }
}

// Global exports
window.openStatusPagesModal = openStatusPagesModal;
window.closeStatusPagesModal = closeStatusPagesModal;
window.switchStatusPageTab = switchStatusPageTab;
window.openCreateStatusPageForm = openCreateStatusPageForm;
window.openEditStatusPageForm = openEditStatusPageForm;
window.handleStatusPageSubmit = handleStatusPageSubmit;
window.deleteStatusPageConfirm = deleteStatusPageConfirm;

window.openMaintenanceModal = openMaintenanceModal;
window.closeMaintenanceModal = closeMaintenanceModal;
window.switchMaintenanceTab = switchMaintenanceTab;
window.fetchMaintenanceList = fetchMaintenanceList;
window.openCreateMaintenanceForm = openCreateMaintenanceForm;
window.openEditMaintenanceForm = openEditMaintenanceForm;
window.handleMaintenanceSubmit = handleMaintenanceSubmit;
window.deleteMaintenanceConfirm = deleteMaintenanceConfirm;
window.toggleAllMaintenanceMonitors = toggleAllMaintenanceMonitors;

window.toggleTemplateMode = toggleTemplateMode;
window.updateTemplatePreview = updateTemplatePreview;
window.submitWebhookForm = submitWebhookForm;
window.handleWebhookSubmit = submitWebhookForm;
window.testWebhook = testWebhook;
window.triggerTestNotification = testWebhook;
window.openEditWebhookModal = openEditWebhookModal;
window.openEditMonitorModal = openEditMonitorModal;
window.switchModalTab = switchModalTab;
window.closeModal = closeModal;
window.resetWebhookForm = resetWebhookForm;
window.deleteWebhook = deleteWebhook;

document.addEventListener('DOMContentLoaded', () => {
  const providerSelect = document.getElementById('w-provider');
  if (providerSelect) {
    providerSelect.addEventListener('change', updateTemplatePreview);
  }

  const { backdrop } = getModalElements();
  if (backdrop) {
    backdrop.addEventListener('click', (e) => {
      if (e.target === backdrop) {
        closeModal();
      }
    });
  }

  const spBackdrop = document.getElementById('status-pages-modal-backdrop');
  if (spBackdrop) {
    spBackdrop.addEventListener('click', (e) => {
      if (e.target === spBackdrop) {
        closeStatusPagesModal();
      }
    });
  }

  const maintBackdrop = document.getElementById('maintenance-modal-backdrop');
  if (maintBackdrop) {
    maintBackdrop.addEventListener('click', (e) => {
      if (e.target === maintBackdrop) {
        closeMaintenanceModal();
      }
    });
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      const { backdrop: modalEl } = getModalElements();
      if (modalEl && !modalEl.classList.contains('hidden')) {
        closeModal();
      }
      const logsModal = document.getElementById('logs-modal');
      if (logsModal && !logsModal.classList.contains('hidden')) {
        closeLogsModal();
      }
      const spModal = document.getElementById('status-pages-modal-backdrop');
      if (spModal && !spModal.classList.contains('hidden')) {
        closeStatusPagesModal();
      }
      const maintModal = document.getElementById('maintenance-modal-backdrop');
      if (maintModal && !maintModal.classList.contains('hidden')) {
        closeMaintenanceModal();
      }
      const badgeModal = document.getElementById('badge-modal');
      if (badgeModal && !badgeModal.classList.contains('hidden') && typeof closeBadgeModal === 'function') {
        closeBadgeModal();
      }
    }
  });
});
