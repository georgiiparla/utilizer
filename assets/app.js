// Installed PWAs bypass the command-line window size. Apply the default once,
// then let the browser remember the user's own resizing.
if (window.matchMedia('(display-mode: standalone)').matches) {
  try {
    if (!localStorage.getItem('utilizer-window-size-v1')) {
      window.resizeTo(Math.min(960, screen.availWidth), Math.min(720, screen.availHeight));
      localStorage.setItem('utilizer-window-size-v1', 'set');
    }
  } catch (_) {
    // Storage or window management may be restricted by the browser.
  }
}

// One shared notification, with queued messages while an important one is open.
let noticeTimer;
let importantNotice = false;
const noticeQueue = [];
function notify(message, important = false) {
  const host = document.getElementById('feedback');
  if (importantNotice) {
    if (host.dataset.message !== message && !noticeQueue.some(item => item.message === message) && noticeQueue.length < 5) noticeQueue.push({ message, important });
    return;
  }
  clearTimeout(noticeTimer);
  const text = document.createElement('span');
  text.textContent = message;
  host.replaceChildren(text);
  host.dataset.message = message;
  host.setAttribute('role', important ? 'alert' : 'status');
  importantNotice = important;
  if (!important) {
    const progress = document.createElement('span'); progress.className = 'notice-duration'; progress.setAttribute('aria-hidden', 'true');
    host.append(progress);
    noticeTimer = setTimeout(dismissNotice, 3000);
  }
  const dialog = document.querySelector('#editor[open]');
  if (dialog && !dialog.contains(host)) dialog.append(host);
  host.showPopover();
}
function dismissNotice() {
  clearTimeout(noticeTimer);
  document.getElementById('feedback').hidePopover();
  importantNotice = false;
  const next = noticeQueue.shift();
  if (next) notify(next.message, next.important);
}
function connectionError() { notify('Connection lost', true); }
document.addEventListener('htmx:sendError', connectionError);
document.addEventListener('htmx:timeout', connectionError);
document.addEventListener('htmx:responseError', function (event) {
  if (event.detail.xhr.getResponseHeader('HX-Retarget') !== '#feedback') notify('Request failed', true);
});

function viewState() {
  return JSON.parse(document.getElementById('locations').getAttribute('data-state'));
}
let lastSelection = 0;
let openingEditor = false;
let syncing = false;
function selectSpot(point) {
  const locations = document.getElementById('locations');
  locations.dataset.point = point;
  locations.querySelectorAll('.spot, .dot').forEach(function (element) {
    element.classList.toggle('selected', element.dataset.point === point);
  });
  const state = viewState(); state.point = point;
  locations.setAttribute('data-state', JSON.stringify(state));
  delete state.revision;
  history.replaceState(null, '', '/?' + new URLSearchParams(state));
  lastSelection = Date.now();
}
function openEditor(kind, point) {
  if (openingEditor) return;
  const params = new URLSearchParams({ kind, map: document.getElementById('locations').dataset.map });
  if (point) params.set('point', point);
  openingEditor = true;
  htmx.ajax('GET', '/editor?' + params, { target: '#editor-host', swap: 'innerHTML' })
    .catch(connectionError).finally(function () { openingEditor = false; });
}
function removeEditor() {
  const dialog = document.getElementById('editor');
  const feedback = document.getElementById('feedback');
  const visible = feedback.matches(':popover-open');
  if (dialog && dialog.contains(feedback)) {
    feedback.hidePopover(); document.body.append(feedback);
  }
  if (dialog) dialog.close();
  document.getElementById('editor-host').replaceChildren();
  if (visible) feedback.showPopover();
}
function closeEditor() {
  const dialog = document.getElementById('editor');
  if (!dialog || dialog.querySelector('[aria-busy="true"]')) return;
  removeEditor();
  document.body.dispatchEvent(new Event('locationsChanged'));
}
function syncPage() {
  if (!document.getElementById('locations')) return;
  if (syncing) return;
  if (document.querySelector('.editor-form[data-dirty="true"]')) { notify('Save changes first', true); return; }
  const locations = document.getElementById('locations');
  syncing = true;
  htmx.ajax('POST', '/manage', { target: '#feedback', swap: 'innerHTML', values: {
    kind: 'sync', map: locations.dataset.map, mode: locations.dataset.mode, point: locations.dataset.point
  } }).catch(connectionError).finally(function () { syncing = false; });
}

document.addEventListener('change', function (event) {
  if (event.target.matches('[data-map-select], [data-mode-select]')) event.target.form.requestSubmit();
});
let backdropPress = false;
function outsideDialog(event, dialog) {
  const bounds = dialog.getBoundingClientRect();
  return event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom;
}
document.addEventListener('pointerdown', function (event) {
  const feedback = document.getElementById('feedback');
  if (feedback.matches(':popover-open') && !feedback.contains(event.target)) dismissNotice();
  const dialog = document.querySelector('#editor[open]');
  backdropPress = !!dialog && event.target === dialog && outsideDialog(event, dialog);
});
document.addEventListener('click', function (event) {
  const dialog = document.querySelector('#editor[open]');
  const close = backdropPress && dialog && event.target === dialog && outsideDialog(event, dialog);
  backdropPress = false;
  if (close) { closeEditor(); return; }
  const launch = event.target.closest('[data-editor]');
  if (launch) { event.preventDefault(); openEditor(launch.dataset.editor, launch.dataset.point); return; }
  const point = event.target.closest('.spot[data-point], .dot[data-point]');
  if (point) { event.preventDefault(); selectSpot(point.dataset.point); }
});
document.addEventListener('dblclick', function (event) {
  const spot = event.target.closest('.spot[data-point]');
  if (spot) { event.preventDefault(); openEditor('spot', spot.dataset.point); }
});
document.addEventListener('keydown', function (event) {
  const feedback = document.getElementById('feedback');
  if (feedback.matches(':popover-open') && !feedback.contains(event.target) && !['Control', 'Shift', 'Alt', 'Meta'].includes(event.key)) dismissNotice();
  if (event.ctrlKey && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 's') {
    event.preventDefault(); if (!event.repeat) syncPage(); return;
  }
  const mapAction = event.target.closest('svg [role="button"]');
  if (mapAction && ['Enter', ' '].includes(event.key)) {
    event.preventDefault(); htmx.trigger(mapAction, 'click'); return;
  }
  const spot = event.target.closest('.spot[data-point]');
  if (spot && event.key === 'Enter') { event.preventDefault(); openEditor('spot', spot.dataset.point); }
  if (spot && ['ArrowUp', 'ArrowDown'].includes(event.key)) {
    event.preventDefault();
    const spots = Array.from(document.querySelectorAll('nav .spot'));
    const next = spots[(spots.indexOf(spot) + (event.key === 'ArrowDown' ? 1 : spots.length - 1)) % spots.length];
    next.focus(); selectSpot(next.dataset.point);
  }
});
document.addEventListener('input', function (event) {
  const form = event.target.closest('.editor-form');
  if (form) form.dataset.dirty = 'true';
});
document.addEventListener('htmx:configRequest', function (event) {
  if (event.detail.elt.id === 'locations') Object.assign(event.detail.parameters, viewState());
});
document.addEventListener('htmx:beforeRequest', function (event) {
  const { elt, target, xhr } = event.detail;
  if (elt.id === 'locations') {
    if (openingEditor || document.querySelector('#editor[open]') || Date.now() - lastSelection < 600) { event.preventDefault(); return; }
    xhr.viewState = document.getElementById('locations').getAttribute('data-state');
  }
  if (target.id === 'locations') {
    xhr.listScroll = document.querySelector('nav').scrollTop;
    xhr.lastSelection = lastSelection;
  }
  const form = elt.closest('.editor-form');
  if (form) {
    if (form.getAttribute('aria-busy') === 'true') { event.preventDefault(); return; }
    form.setAttribute('aria-busy', 'true');
    form.querySelectorAll('button').forEach(button => { button.disabled = true; });
  }
});
document.addEventListener('htmx:beforeSwap', function (event) {
  const { xhr, target } = event.detail;
  // A response started before a local click must not replace the newer selection.
  if (target.id === 'locations' && (openingEditor || document.querySelector('#editor[open]') || xhr.lastSelection !== lastSelection)) {
    event.detail.shouldSwap = false; return;
  }
  if (xhr.viewState && (xhr.viewState !== document.getElementById('locations').getAttribute('data-state') || document.querySelector('#editor[open]'))) {
    event.detail.shouldSwap = false; return;
  }
  if (xhr.status >= 400 && xhr.getResponseHeader('HX-Retarget') === '#feedback') event.detail.shouldSwap = true;
  if (target.id === 'feedback') {
    // Keep an important notification visible while the response is processed.
    event.detail.shouldSwap = false;
    const fragment = new DOMParser().parseFromString(event.detail.serverResponse, 'text/html');
    const saved = fragment.querySelector('[data-saved]');
    const important = xhr.status >= 400 || !!fragment.querySelector('[data-important]');
    notify(fragment.body.textContent.trim(), important);
    if (saved && saved.dataset.changed === 'true') {
      removeEditor();
      const state = viewState(); delete state.revision;
      htmx.ajax('GET', '/locations?' + new URLSearchParams(state), { target: '#locations', swap: 'outerHTML' }).catch(connectionError);
    }
  }
});
document.addEventListener('htmx:afterRequest', function (event) {
  const form = event.detail.elt.closest('.editor-form');
  if (form) {
    form.removeAttribute('aria-busy');
    form.querySelectorAll('button').forEach(button => { button.disabled = false; });
  }
});
document.addEventListener('htmx:afterSwap', function (event) {
  if (event.detail.target.id === 'editor-host') {
    const dialog = document.getElementById('editor');
    if (dialog) {
      dialog.showModal();
      const feedback = document.getElementById('feedback');
      if (feedback.matches(':popover-open')) { feedback.hidePopover(); dialog.append(feedback); feedback.showPopover(); }
      dialog.addEventListener('cancel', function (cancel) { cancel.preventDefault(); closeEditor(); });
    }
  }
  if (event.detail.target.id === 'locations' && event.detail.xhr.viewState) document.querySelector('nav').scrollTop = event.detail.xhr.listScroll;
});

const initialNotice = document.querySelector('#feedback [data-important]');
if (initialNotice) notify(initialNotice.textContent, true);
