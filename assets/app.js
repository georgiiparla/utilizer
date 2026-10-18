document.documentElement.classList.add('js');
document.addEventListener('change', function (event) {
  if (event.target.matches('[data-submit-change]')) event.target.form.requestSubmit();
});
document.addEventListener('click', async function (event) {
  const button = event.target.closest('[data-copy]');
  if (!button) return;
  const text = document.getElementById(button.dataset.copy).textContent;
  try {
    await navigator.clipboard.writeText(text);
    button.textContent = 'Copied';
    setTimeout(function () { if (button.isConnected) button.textContent = 'Copy'; }, 1500);
  } catch (_) {
    const selection = window.getSelection();
    const range = document.createRange();
    range.selectNodeContents(document.getElementById(button.dataset.copy));
    selection.removeAllRanges(); selection.addRange(range);
    button.textContent = 'Press Ctrl+C';
  }
});
function connectionError() {
  const feedback = document.getElementById('feedback');
  if (feedback) feedback.textContent = 'Cannot reach Utilizer. Check that its console is still open.';
}
document.addEventListener('htmx:sendError', connectionError);
document.addEventListener('htmx:timeout', connectionError);

document.addEventListener('htmx:beforeSwap', function (event) {
  if (event.detail.xhr.status < 400) return;
  // Only our escaped error fragment may replace feedback. Framework errors
  // (invalid forms, oversized requests) must never replace the player/inbox.
  if (event.detail.xhr.getResponseHeader('HX-Retarget') === '#feedback') {
    event.detail.shouldSwap = true;
  } else {
    event.detail.shouldSwap = false;
    const feedback = document.getElementById('feedback');
    if (feedback) feedback.textContent = 'Cannot complete this request. Refresh the player and try again.';
  }
});
