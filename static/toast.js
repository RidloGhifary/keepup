// ── KeepUp Apple-Style Toast Notification & Centralized API Fetcher ─────────

/**
 * Ensures the container element for toasts exists in the DOM.
 */
function getOrCreateToastContainer() {
  let container = document.getElementById('toast-container');
  if (!container) {
    container = document.createElement('div');
    container.id = 'toast-container';
    container.className =
      'fixed top-6 right-6 z-50 flex flex-col gap-3 pointer-events-none max-w-sm w-full px-4 sm:px-0';
    document.body.appendChild(container);
  }
  return container;
}

/**
 * Display an Apple-styled glassmorphic toast notification.
 *
 * Supports both signatures:
 * - showToast({ title, message, type, duration })
 * - showToast(message, type)
 */
function showToast(options, legacyType = 'info') {
  let title = '';
  let message = '';
  let type = 'info';
  let duration = 4000;

  if (typeof options === 'string') {
    message = options;
    type = legacyType || 'info';
    title =
      type === 'error'
        ? 'Error'
        : type === 'warning'
        ? 'Warning'
        : type === 'success'
        ? 'Success'
        : 'Notice';
  } else if (typeof options === 'object' && options !== null) {
    message = options.message || '';
    type = options.type || 'info';
    duration = options.duration || 4000;
    title =
      options.title ||
      (type === 'error'
        ? 'Error'
        : type === 'warning'
        ? 'Warning'
        : type === 'success'
        ? 'Success'
        : 'Notice');
  }

  const container = getOrCreateToastContainer();
  const toast = document.createElement('div');

  // Palette & SVG icon configuration matching Apple HIG
  let iconSvg = '';
  let badgeStyle = '';

  switch (type) {
    case 'error':
      badgeStyle =
        'bg-[#ff453a]/15 border border-[#ff453a]/30 text-[#ff453a] shadow-[0_0_12px_rgba(255,69,58,0.25)]';
      iconSvg = `
        <svg class="w-4 h-4 text-[#ff453a]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
        </svg>`;
      break;
    case 'warning':
      badgeStyle =
        'bg-[#ffd60a]/15 border border-[#ffd60a]/30 text-[#ffd60a] shadow-[0_0_12px_rgba(255,214,10,0.25)]';
      iconSvg = `
        <svg class="w-4 h-4 text-[#ffd60a]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>
        </svg>`;
      break;
    case 'success':
      badgeStyle =
        'bg-[#30d158]/15 border border-[#30d158]/30 text-[#30d158] shadow-[0_0_12px_rgba(48,209,88,0.25)]';
      iconSvg = `
        <svg class="w-4 h-4 text-[#30d158]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"/>
        </svg>`;
      break;
    default:
      badgeStyle =
        'bg-[#0a84ff]/15 border border-[#0a84ff]/30 text-[#0a84ff] shadow-[0_0_12px_rgba(10,132,255,0.25)]';
      iconSvg = `
        <svg class="w-4 h-4 text-[#0a84ff]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>
        </svg>`;
      break;
  }

  toast.className =
    'pointer-events-auto bg-neutral-900/90 backdrop-blur-2xl border border-white/10 rounded-2xl p-4 shadow-[0_10px_30px_rgba(0,0,0,0.5)] flex items-start gap-3 transition-all duration-300 ease-out transform translate-y-2 opacity-0';

  toast.innerHTML = `
    <div class="h-8 w-8 rounded-xl flex items-center justify-center shrink-0 ${badgeStyle}">
      ${iconSvg}
    </div>
    <div class="flex-1 min-w-0 pt-0.5">
      <h4 class="font-semibold text-white text-xs sm:text-sm tracking-tight leading-tight">${escapeHtml(title)}</h4>
      <p class="text-xs text-neutral-300 mt-1 leading-relaxed break-words">${escapeHtml(message)}</p>
    </div>
    <button type="button" class="toast-close-btn text-white/40 hover:text-white p-1 rounded-lg hover:bg-white/5 transition shrink-0" title="Close">
      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
      </svg>
    </button>
  `;

  // Dismiss handler
  const dismiss = () => {
    toast.classList.remove('opacity-100', 'translate-y-0');
    toast.classList.add('opacity-0', 'translate-y-2');
    setTimeout(() => {
      if (toast.parentNode) toast.parentNode.removeChild(toast);
    }, 300);
  };

  toast.querySelector('.toast-close-btn').addEventListener('click', dismiss);
  container.appendChild(toast);

  // Trigger smooth spring animation
  requestAnimationFrame(() => {
    toast.classList.remove('translate-y-2', 'opacity-0');
    toast.classList.add('translate-y-0', 'opacity-100');
  });

  // Auto-dismiss after configured duration
  const timer = setTimeout(dismiss, duration);
  toast.addEventListener('mouseenter', () => clearTimeout(timer));

  return toast;
}

/**
 * Escapes unsafe characters for HTML injection.
 */
function escapeHtml(str) {
  if (typeof str !== 'string') return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

// ── Centralized API Fetcher (apiFetch) ────────────────────────────────────────

let isRedirectingToLogin = false;

/**
 * Centralized fetch wrapper handling 401 Unauthorized, 429 Rate Limiting,
 * and 5xx Server Errors with Apple-style toast alerts.
 */
async function apiFetch(url, options = {}) {
  try {
    options.credentials = options.credentials || 'include';
    const res = await fetch(url, options);

    // ── 401 Unauthorized ──────────────────────────────────────────────────────
    if (res.status === 401) {
      const isLoginPage =
        window.location.pathname.endsWith('/login.html') ||
        window.location.pathname === '/login';
      if (!isLoginPage) {
        if (!url.includes('/api/auth/login')) {
          showToast({
            title: 'Session Expired',
            message: 'Please log in again to continue.',
            type: 'error',
            duration: 3500,
          });
        }

        if (!isRedirectingToLogin) {
          isRedirectingToLogin = true;
          window.location.href = '/login.html';
        }
      }
      return res;
    }

    // ── 429 Too Many Requests ─────────────────────────────────────────────────
    if (res.status === 429) {
      let errorMessage = 'Too many attempts. Please wait a moment before trying again.';
      try {
        const cloned = res.clone();
        const data = await cloned.json();
        if (data && data.error) {
          errorMessage = data.error;
        }
      } catch (_) {}

      showToast({
        title: 'Rate Limit Exceeded',
        message: errorMessage,
        type: 'warning',
        duration: 5000,
      });
      return res;
    }

    // ── 5xx Server Errors ─────────────────────────────────────────────────────
    if (res.status >= 500) {
      showToast({
        title: 'Server Error',
        message: 'Unable to reach KeepUp backend.',
        type: 'error',
        duration: 4000,
      });
      return res;
    }

    return res;
  } catch (err) {
    showToast({
      title: 'Server Error',
      message: 'Unable to reach KeepUp backend.',
      type: 'error',
      duration: 4000,
    });
    throw err;
  }
}

// Expose globally on window
window.showToast = showToast;
window.apiFetch = apiFetch;

// Ensure container exists once DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', getOrCreateToastContainer);
} else {
  getOrCreateToastContainer();
}
