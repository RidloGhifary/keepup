// ── Apple-style Modal Controller for KeepUp ──────────────────────────────

let cachedWebhooks = [];

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
  const backdrop = document.getElementById('apple-modal-backdrop');
  const card = document.getElementById('apple-modal-card');

  backdrop.classList.remove('hidden');
  // Trigger smooth Apple-style scale & opacity transition
  requestAnimationFrame(() => {
    card.classList.remove('scale-95', 'opacity-0');
    card.classList.add('scale-100', 'opacity-100');
  });
}

function closeModal() {
  const backdrop = document.getElementById('apple-modal-backdrop');
  const card = document.getElementById('apple-modal-card');

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

  const backdrop = document.getElementById('apple-modal-backdrop');
  if (backdrop) {
    backdrop.addEventListener('click', (e) => {
      if (e.target === backdrop) {
        closeModal();
      }
    });
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      const appleModal = document.getElementById('apple-modal-backdrop');
      if (appleModal && !appleModal.classList.contains('hidden')) {
        closeModal();
      }
      const logsModal = document.getElementById('logs-modal');
      if (logsModal && !logsModal.classList.contains('hidden')) {
        closeLogsModal();
      }
    }
  });
});
