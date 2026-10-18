// Run before CSS so a saved dark theme does not flash white on reload.
(function () {
  let theme = 'light';
  try {
    if (localStorage.getItem('utilizer-theme') === 'dark') theme = 'dark';
  } catch (_) { /* Theme switching still works without persistent storage. */ }

  function sync() {
    document.documentElement.dataset.theme = theme;
    document.querySelectorAll('[data-theme-select]').forEach(function (select) {
      select.value = theme;
    });
  }

  sync();
  document.addEventListener('DOMContentLoaded', sync);
  document.addEventListener('htmx:afterSwap', sync);
  document.addEventListener('change', function (event) {
    if (!event.target.matches('[data-theme-select]')) return;
    theme = event.target.value === 'dark' ? 'dark' : 'light';
    sync();
    try { localStorage.setItem('utilizer-theme', theme); } catch (_) { /* Optional. */ }
  });
})();
