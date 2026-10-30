// ── KeepUp Login Controller ───────────────────────────────────────────────

// Check if already authenticated on load
(async () => {
  try {
    const res = await fetch('/api/auth/me', { credentials: 'include' });
    if (res && res.ok) {
      const data = await res.json();
      if (data && data.authenticated) {
        window.location.replace('/');
      }
    }
  } catch (err) {
    // Not authenticated or network error; stay on login page
  }
})();

async function handleLogin(e) {
  if (e) e.preventDefault();
  const usernameInput = document.getElementById('username');
  const passwordInput = document.getElementById('password');
  const errorBanner = document.getElementById('error-banner');
  const errorMessage = document.getElementById('error-message');
  const submitBtn = document.getElementById('submit-btn');
  const submitText = document.getElementById('submit-text');
  const submitSpinner = document.getElementById('submit-spinner');

  if (errorBanner) errorBanner.classList.add('hidden');
  if (submitBtn) submitBtn.disabled = true;
  if (submitText) submitText.textContent = 'Authenticating...';
  if (submitSpinner) submitSpinner.classList.remove('hidden');

  try {
    const res = await apiFetch('/api/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      credentials: 'include',
      body: JSON.stringify({
        username: usernameInput ? usernameInput.value.trim() : '',
        password: passwordInput ? passwordInput.value : '',
      }),
    });

    if (res && res.ok) {
      if (submitText) submitText.textContent = 'Success! Redirecting...';
      if (typeof showToast === 'function') {
        showToast({
          title: 'Welcome Back',
          message: 'Signed in successfully. Redirecting...',
          type: 'success',
        });
      }
      // Redirect to root dashboard
      setTimeout(() => {
        window.location.replace('/');
      }, 400);
    } else if (res) {
      const data = await res.json().catch(() => ({}));
      const msg =
        data.error ||
        (res.status === 429
          ? 'Too many attempts. Please try again later.'
          : 'Invalid username or password');
      if (errorMessage) errorMessage.textContent = msg;
      if (errorBanner) errorBanner.classList.remove('hidden');
      if (passwordInput) {
        passwordInput.value = '';
        passwordInput.focus();
      }
    } else {
      if (errorMessage) errorMessage.textContent = 'Connection error. Please check server status.';
      if (errorBanner) errorBanner.classList.remove('hidden');
    }
  } catch (err) {
    if (errorMessage) errorMessage.textContent = 'Connection error. Please check server status.';
    if (errorBanner) errorBanner.classList.remove('hidden');
  } finally {
    if (submitBtn) submitBtn.disabled = false;
    if (submitText && submitText.textContent !== 'Success! Redirecting...') {
      submitText.textContent = 'Sign In';
    }
    if (submitSpinner) submitSpinner.classList.add('hidden');
  }
}
