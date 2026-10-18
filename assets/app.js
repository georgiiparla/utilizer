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
document.addEventListener('htmx:sendError', function () {
  const feedback = document.getElementById('feedback');
  if (feedback) feedback.textContent = 'Cannot reach Utilizer. Check that its console is still open.';
});
