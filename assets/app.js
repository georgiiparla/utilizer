// Installed PWAs bypass the command-line window size. Apply the default once,
// then let the browser remember the user's own resizing.
if (window.matchMedia('(display-mode: standalone)').matches) {
  try {
    if (!localStorage.getItem('utilizer-window-size-v2')) {
      const size = Math.min(760, screen.availWidth, screen.availHeight);
      window.resizeTo(size, size);
      localStorage.setItem('utilizer-window-size-v2', 'set');
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
// Keep checking even while map polling is paused by a dialog or interaction.
// Reloading after the local server exits lets the browser show its offline page.
let checkingServer = false;
let reloadingPage = false;
async function checkServerConnection() {
  if (checkingServer || reloadingPage) return;
  checkingServer = true;
  try {
    await fetch('/manifest.webmanifest', {
      method: 'HEAD', cache: 'no-store', signal: AbortSignal.timeout(2000)
    });
  } catch (error) {
    // A busy server is not evidence that the process has exited.
    if (error.name !== 'TimeoutError' && error.name !== 'AbortError') {
      reloadingPage = true;
      window.location.reload();
    }
  } finally {
    checkingServer = false;
  }
}
setInterval(checkServerConnection, 1000);
window.addEventListener('pageshow', checkServerConnection);
document.addEventListener('visibilitychange', function () {
  if (!document.hidden) checkServerConnection();
});
function connectionError() { notify('Connection lost', true); checkServerConnection(); }
document.addEventListener('htmx:sendError', connectionError);
document.addEventListener('htmx:timeout', connectionError);
document.addEventListener('htmx:responseError', function (event) {
  if (event.detail.xhr.getResponseHeader('HX-Retarget') !== '#feedback') notify('Request failed', true);
});

// Keep navigation local and per map, including across polled HTML replacements.
const mapViews = new Map();
let mapDrag = null;
let mapNavigationController;
let mapMarkerObserver;
function groupMapMarkers(points, scale) {
  // Work in screen pixels, so zoom reveals detail. Limit each group's extent:
  // a chain of nearby positions must not collapse a whole corridor into one pin.
  const distance = 48 / scale;
  const groups = [];
  const cells = new Map();
  const ordered = [...points].sort((a, b) => a.x - b.x || a.y - b.y || a.key.localeCompare(b.key));
  for (const point of ordered) {
    const cellX = Math.floor(point.x / distance);
    const cellY = Math.floor(point.y / distance);
    let nearest;
    let nearestDistance = distance;
    for (let x = cellX - 1; x <= cellX + 1; x++) {
      for (let y = cellY - 1; y <= cellY + 1; y++) {
        for (const group of cells.get(`${x},${y}`) || []) {
          // Buckets use the fixed seed, keeping both lookup and reach bounded.
          if (Math.hypot(point.x - group.seed.x, point.y - group.seed.y) > distance) continue;
          const apart = Math.hypot(point.x - group.x, point.y - group.y);
          const width = Math.max(point.x, group.maxX) - Math.min(point.x, group.minX);
          const height = Math.max(point.y, group.maxY) - Math.min(point.y, group.minY);
          if (apart <= nearestDistance && Math.hypot(width, height) * scale <= 80) {
            nearest = group;
            nearestDistance = apart;
          }
        }
      }
    }
    if (nearest) {
      nearest.points.push(point);
      nearest.x += (point.x - nearest.x) / nearest.points.length;
      nearest.y += (point.y - nearest.y) / nearest.points.length;
      nearest.minX = Math.min(nearest.minX, point.x);
      nearest.minY = Math.min(nearest.minY, point.y);
      nearest.maxX = Math.max(nearest.maxX, point.x);
      nearest.maxY = Math.max(nearest.maxY, point.y);
    } else {
      const group = { points: [point], seed: point, x: point.x, y: point.y,
        minX: point.x, maxX: point.x, minY: point.y, maxY: point.y };
      const key = `${cellX},${cellY}`;
      if (!cells.has(key)) cells.set(key, []);
      cells.get(key).push(group);
      groups.push(group);
    }
  }
  return mergeOverlappingMarkers(groups, scale);
}
function mapClusterRadius(group, scale) {
  const spread = Math.hypot(group.maxX - group.minX, group.maxY - group.minY) * scale;
  // Logarithmic growth keeps a dense stack readable without covering the map.
  return Math.min(24, 14 + Math.log2(group.points.length) * 1.25 + Math.min(3, spread / 24));
}
function mergeOverlappingMarkers(groups, scale) {
  // Reconcile circles whose weighted centers moved toward each other. Keep the
  // a bounded extent, so fixing badge overlap cannot create a map-wide chain.
  const cells = new Map();
  const active = new Set(groups);
  const cellSize = 64 / scale;
  const cellKey = group => `${Math.floor(group.x / cellSize)},${Math.floor(group.y / cellSize)}`;
  const radius = group => group.points.length === 1 ? 5 : mapClusterRadius(group, scale);
  function insert(group) {
    const key = cellKey(group);
    if (!cells.has(key)) cells.set(key, new Set());
    cells.get(key).add(group);
  }
  groups.forEach(insert);
  for (const group of groups) {
    if (!active.has(group)) continue;
    let merged;
    do {
      merged = false;
      const cellX = Math.floor(group.x / cellSize);
      const cellY = Math.floor(group.y / cellSize);
      search: for (let x = cellX - 1; x <= cellX + 1; x++) {
        for (let y = cellY - 1; y <= cellY + 1; y++) {
          for (const other of cells.get(`${x},${y}`) || []) {
            if (other === group) continue;
            const width = Math.max(group.maxX, other.maxX) - Math.min(group.minX, other.minX);
            const height = Math.max(group.maxY, other.maxY) - Math.min(group.minY, other.minY);
            // Allow a little more reach than the initial grouping for dense edges.
            if (Math.hypot(width, height) * scale > 128) continue;
            if (Math.hypot(group.x - other.x, group.y - other.y) * scale >= radius(group) + radius(other) + 3) continue;
            cells.get(cellKey(group)).delete(group);
            cells.get(cellKey(other)).delete(other);
            active.delete(other);
            const count = group.points.length + other.points.length;
            group.x += (other.x - group.x) * other.points.length / count;
            group.y += (other.y - group.y) * other.points.length / count;
            group.points = group.points.concat(other.points);
            group.minX = Math.min(group.minX, other.minX);
            group.minY = Math.min(group.minY, other.minY);
            group.maxX = Math.max(group.maxX, other.maxX);
            group.maxY = Math.max(group.maxY, other.maxY);
            insert(group);
            merged = true;
            break search;
          }
        }
      }
    } while (merged);
  }
  return [...active];
}
function unfoldMapMarkers(points, scale) {
  const cells = new Map();
  const spacing = 24;
  return [...points].sort((a, b) => a.index - b.index).map(point => {
    const origin = { x: point.x * scale, y: point.y * scale };
    function free(x, y) {
      const cx = Math.floor(x / spacing), cy = Math.floor(y / spacing);
      for (let dx = -1; dx <= 1; dx++) {
        for (let dy = -1; dy <= 1; dy++) {
          for (const other of cells.get(`${cx + dx},${cy + dy}`) || []) {
            if (Math.hypot(x - other.x, y - other.y) < spacing - .001) return false;
          }
        }
      }
      return true;
    }
    let x = origin.x, y = origin.y;
    // Only overlapping hit targets move. A short leader retains the real position.
    search: for (let ring = 0; !free(x, y); ring++) {
      const radius = ring + 1;
      const offsets = [];
      for (let dx = -radius; dx <= radius; dx++) {
        for (let dy = -radius; dy <= radius; dy++) {
          if (Math.max(Math.abs(dx), Math.abs(dy)) !== radius) continue;
          offsets.push({ dx, dy });
        }
      }
      offsets.sort((a, b) => a.dx * a.dx + a.dy * a.dy - b.dx * b.dx - b.dy * b.dy);
      for (const { dx, dy } of offsets) {
        x = origin.x + dx * spacing;
        y = origin.y + dy * spacing;
        if (free(x, y)) break search;
      }
    }
    const key = `${Math.floor(x / spacing)},${Math.floor(y / spacing)}`;
    if (!cells.has(key)) cells.set(key, []);
    cells.get(key).push({ x, y });
    return { point, x: x / scale, y: y / scale };
  });
}
function initializeMapNavigation() {
  const svg = document.querySelector('[data-map-viewport]');
  if (!svg || svg.dataset.navigationReady) return;
  svg.dataset.navigationReady = 'true';
  const map = svg.closest('[data-map]').dataset.map;
  const base = { x: svg.viewBox.baseVal.x, y: svg.viewBox.baseVal.y, width: svg.viewBox.baseVal.width, height: svg.viewBox.baseVal.height };
  const view = mapViews.get(map) || { ...base };
  mapViews.set(map, view);
  const controls = svg.closest('#locations').querySelector('.map-controls');
  const symbols = svg.querySelectorAll('.dot-symbol');
  const points = Array.from(svg.querySelectorAll('.dot'), (node, index) => {
    const position = node.transform.baseVal.consolidate().matrix;
    return { node, key: node.dataset.point, index, x: position.e, y: position.f };
  });
  const clusterLayer = document.createElementNS(svg.namespaceURI, 'g');
  clusterLayer.classList.add('marker-clusters');
  svg.append(clusterLayer);
  const leaders = document.createElementNS(svg.namespaceURI, 'g');
  leaders.classList.add('marker-leaders');
  svg.insertBefore(leaders, points[0]?.node || clusterLayer);
  let markerScale;
  let clusters = [];
  const clamp = (value, min, max) => Math.max(min, Math.min(max, value));
  function focusMarker(key) {
    const cluster = clusters.find(({ group }) => group.points.some(point => point.key === key));
    (cluster?.marker || points.find(point => point.key === key)?.node)?.focus({ preventScroll: true });
  }
  function sizeMarkers() {
    const matrix = svg.getScreenCTM();
    if (!matrix) return;
    const scale = Math.hypot(matrix.a, matrix.b);
    if (scale <= 0) return;
    // Panning and selection keep the same groups; only scale/data changes regroup.
    if (scale !== markerScale) {
      const focusedMarker = document.activeElement?.closest('.dot, .marker-cluster');
      const focusedKey = focusedMarker?.dataset.point || focusedMarker?.dataset.firstPoint;
      markerScale = scale;
      symbols.forEach(symbol => symbol.setAttribute('transform', `scale(${1 / scale})`));
      points.forEach(point => {
        point.node.classList.remove('clustered');
        point.node.setAttribute('transform', `translate(${point.x} ${point.y})`);
      });
      leaders.replaceChildren();
      clusterLayer.replaceChildren();
      clusters = [];
      if (svg.closest('[data-mode]').dataset.mode === 'spots' && view.width <= base.width / 8 + .001) {
        for (const { point, x, y } of unfoldMapMarkers(points, scale)) {
          point.node.setAttribute('transform', `translate(${x} ${y})`);
          if (Math.hypot(x - point.x, y - point.y) < .001) continue;
          const line = document.createElementNS(svg.namespaceURI, 'line');
          line.setAttribute('x1', point.x); line.setAttribute('y1', point.y);
          line.setAttribute('x2', x); line.setAttribute('y2', y);
          leaders.append(line);
        }
      } else if (svg.closest('[data-mode]').dataset.mode === 'spots') {
        for (const group of groupMapMarkers(points, scale)) {
          if (group.points.length === 1) continue;
          group.points.forEach(point => point.node.classList.add('clustered'));
          const marker = document.createElementNS(svg.namespaceURI, 'g');
          const radius = mapClusterRadius(group, scale);
          const first = group.points.reduce((a, b) => a.index < b.index ? a : b);
          marker.classList.add('marker-cluster');
          marker.setAttribute('role', 'button');
          marker.setAttribute('tabindex', '0');
          marker.setAttribute('aria-label', `${group.points.length} spots. Zoom in to explore`);
          marker.dataset.firstPoint = first.key;
          marker.setAttribute('transform', `translate(${group.x} ${group.y}) scale(${1 / scale})`);
          const circle = document.createElementNS(svg.namespaceURI, 'circle');
          circle.setAttribute('r', radius);
          const label = document.createElementNS(svg.namespaceURI, 'text');
          label.textContent = group.points.length;
          label.setAttribute('dy', '.35em');
          label.setAttribute('font-size', Math.min(14, 12 + (radius - 14) / 5, (radius * 2 - 12) / (label.textContent.length * .65)));
          const title = document.createElementNS(svg.namespaceURI, 'title');
          title.textContent = `${group.points.length} spots`;
          marker.append(title, circle, label);
          marker.addEventListener('click', function (event) {
            event.preventDefault(); event.stopPropagation();
            selectSpot(first.key);
            // Center the group before zooming so edge clusters stay in view.
            view.x = group.x - view.width / 2;
            view.y = group.y - view.height / 2;
            zoom(2, { x: group.x, y: group.y });
          });
          clusterLayer.append(marker);
          clusters.push({ group, marker });
        }
      }
      if (focusedKey) focusMarker(focusedKey);
    }
    for (const { group, marker } of clusters) {
      const selected = group.points.some(point => point.node.classList.contains('selected'));
      marker.classList.toggle('selected', selected);
      marker.setAttribute('aria-label', `${group.points.length} spots${selected ? ', includes selected spot' : ''}. Zoom in to explore`);
      if (selected && marker !== clusterLayer.lastElementChild) {
        const focused = document.activeElement === marker;
        clusterLayer.append(marker);
        if (focused) marker.focus({ preventScroll: true });
      }
    }
  }
  function paint() {
    view.x = clamp(view.x, base.x, base.x + base.width - view.width);
    view.y = clamp(view.y, base.y, base.y + base.height - view.height);
    svg.setAttribute('viewBox', `${view.x} ${view.y} ${view.width} ${view.height}`);
    sizeMarkers();
    controls.querySelector('[data-map-zoom="out"]').disabled = view.width >= base.width;
    controls.querySelector('[data-map-zoom="in"]').disabled = view.width <= base.width / 8;
  }
  function zoom(factor, anchor = { x: view.x + view.width / 2, y: view.y + view.height / 2 }) {
    const width = clamp(view.width / factor, base.width / 8, base.width);
    const ratio = width / view.width;
    view.x = anchor.x - (anchor.x - view.x) * ratio;
    view.y = anchor.y - (anchor.y - view.y) * ratio;
    view.width = width;
    view.height = base.height * width / base.width;
    paint();
  }
  controls.addEventListener('click', function (event) {
    const action = event.target.closest('[data-map-zoom]')?.dataset.mapZoom;
    if (action === 'fit') { Object.assign(view, base); paint(); }
    else if (action) zoom(action === 'in' ? 1.25 : 1 / 1.25);
  });
  svg.addEventListener('wheel', function (event) {
    event.preventDefault();
    if (mapDrag) return;
    const matrix = svg.getScreenCTM();
    if (!matrix) return;
    const delta = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? svg.clientHeight : 1);
    zoom(Math.exp(-clamp(delta, -300, 300) * .002), new DOMPoint(event.clientX, event.clientY).matrixTransform(matrix.inverse()));
  }, { passive: false });
  let suppressClick = false;
  svg.addEventListener('pointerdown', function (event) {
    if (!event.isPrimary || event.button !== 0 || mapDrag) return;
    suppressClick = false;
    const matrix = svg.getScreenCTM();
    if (!matrix) return;
    mapDrag = { svg, id: event.pointerId, x: event.clientX, y: event.clientY, viewX: view.x, viewY: view.y, matrix: matrix.inverse(), moved: false };
  });
  svg.addEventListener('pointermove', function (event) {
    const drag = mapDrag;
    if (!drag || drag.svg !== svg || drag.id !== event.pointerId) return;
    if (!drag.moved && Math.hypot(event.clientX - drag.x, event.clientY - drag.y) < 5) return;
    if (!drag.moved) {
      drag.moved = true;
      svg.setPointerCapture(event.pointerId);
      svg.classList.add('panning');
    }
    const start = new DOMPoint(drag.x, drag.y).matrixTransform(drag.matrix);
    const current = new DOMPoint(event.clientX, event.clientY).matrixTransform(drag.matrix);
    view.x = drag.viewX + start.x - current.x;
    view.y = drag.viewY + start.y - current.y;
    paint();
  });
  function endDrag(event) {
    if (!mapDrag || mapDrag.svg !== svg || mapDrag.id !== event.pointerId) return;
    suppressClick = mapDrag.moved;
    mapDrag = null;
    svg.classList.remove('panning');
    if (svg.hasPointerCapture(event.pointerId)) svg.releasePointerCapture(event.pointerId);
  }
  // Capture only after moving so ordinary marker clicks retain their target.
  if (mapNavigationController) mapNavigationController.abort();
  mapNavigationController = new AbortController();
  const signal = mapNavigationController.signal;
  window.addEventListener('pointerup', endDrag, { signal });
  window.addEventListener('blur', function () {
    if (mapDrag?.svg === svg) endDrag({ pointerId: mapDrag.id });
  }, { signal });
  svg.addEventListener('pointercancel', endDrag);
  svg.addEventListener('lostpointercapture', function (event) {
    // Touch can transfer implicit capture from a marker to the SVG.
    if (event.target === svg) endDrag(event);
  });
  svg.addEventListener('click', function (event) {
    if (!suppressClick || event.detail === 0) return;
    suppressClick = false;
    event.preventDefault(); event.stopImmediatePropagation();
  }, true);
  svg.addEventListener('keydown', function (event) {
    if (event.target !== svg || event.ctrlKey || event.altKey || event.metaKey) return;
    if (!['+', '=', '-', '0', 'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
    event.preventDefault();
    if (event.key === '0') { Object.assign(view, base); paint(); }
    else if (['+', '=', '-'].includes(event.key)) zoom(event.key === '-' ? 1 / 1.25 : 1.25);
    else {
      if (event.key === 'ArrowLeft') view.x -= view.width / 10;
      if (event.key === 'ArrowRight') view.x += view.width / 10;
      if (event.key === 'ArrowUp') view.y -= view.height / 10;
      if (event.key === 'ArrowDown') view.y += view.height / 10;
      paint();
    }
  });
  if (mapMarkerObserver) mapMarkerObserver.disconnect();
  mapMarkerObserver = new ResizeObserver(sizeMarkers);
  mapMarkerObserver.observe(svg);
  svg.addEventListener('markersChanged', sizeMarkers);
  svg.addEventListener('focusMarker', event => focusMarker(event.detail));
  const selectedMarker = svg.querySelector('.dot.selected');
  if (selectedMarker) svg.append(selectedMarker);
  paint();
}
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
  const selectedMarker = locations.querySelector('.dot.selected');
  if (selectedMarker && selectedMarker !== selectedMarker.parentElement.lastElementChild) {
    const focused = document.activeElement === selectedMarker;
    selectedMarker.parentElement.append(selectedMarker);
    if (focused) selectedMarker.focus({ preventScroll: true });
  }
  const state = viewState(); state.point = point;
  locations.setAttribute('data-state', JSON.stringify(state));
  delete state.revision;
  history.replaceState(null, '', '/?' + new URLSearchParams(state));
  lastSelection = Date.now();
  locations.querySelector('[data-map-viewport]')?.dispatchEvent(new Event('markersChanged'));
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

let spotMenuSource;
function spotMenuOpen() {
  return !!document.getElementById('spot-menu')?.matches(':popover-open');
}
function closeSpotMenu(restoreFocus = false) {
  const menu = document.getElementById('spot-menu');
  if (spotMenuOpen()) menu.hidePopover();
  if (menu) delete menu.dataset.position;
  if (restoreFocus && spotMenuSource?.isConnected) spotMenuSource.focus({ preventScroll: true });
}
document.addEventListener('contextmenu', function (event) {
  event.preventDefault();
  closeSpotMenu();
  const point = event.target.closest('.spot[data-point], .dot[data-point]');
  const menu = document.getElementById('spot-menu');
  if (!point || !menu) return;
  const row = document.getElementById('point-' + point.dataset.point);
  if (!row?.dataset.position) return;
  const bounds = point.getBoundingClientRect();
  selectSpot(point.dataset.point);
  spotMenuSource = row;
  menu.dataset.position = row.dataset.position;
  menu.showPopover();
  const x = event.clientX || event.clientY ? event.clientX : bounds.left;
  const y = event.clientX || event.clientY ? event.clientY : bounds.bottom;
  menu.style.left = Math.max(8, Math.min(x, window.innerWidth - menu.offsetWidth - 8)) + 'px';
  menu.style.top = Math.max(8, Math.min(y, window.innerHeight - menu.offsetHeight - 8)) + 'px';
  menu.querySelector('button').focus({ preventScroll: true });
});
document.addEventListener('scroll', function () { closeSpotMenu(); }, true);
window.addEventListener('resize', function () { closeSpotMenu(); });
window.addEventListener('blur', function () { closeSpotMenu(); });

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
  if (event.target.closest('[data-copy-position]')) {
    const command = document.getElementById('spot-menu').dataset.position;
    closeSpotMenu(true);
    if (command) {
      if (!navigator.clipboard?.writeText) {
        notify('Clipboard access is unavailable in this window.', true);
        return;
      }
      navigator.clipboard.writeText(command)
        .then(function () { notify('Position copied'); })
        .catch(function () { notify('Could not copy the position. Check clipboard access.', true); });
    }
    return;
  }
  const dialog = document.querySelector('#editor[open]');
  const close = backdropPress && dialog && event.target === dialog && outsideDialog(event, dialog);
  backdropPress = false;
  if (close) { closeEditor(); return; }
  const launch = event.target.closest('[data-editor]');
  if (launch) { event.preventDefault(); openEditor(launch.dataset.editor, launch.dataset.point); return; }
  const point = event.target.closest('.spot[data-point], .dot[data-point]');
  if (point) { event.preventDefault(); selectSpot(point.dataset.point); return; }
});
document.addEventListener('dblclick', function (event) {
  const spot = event.target.closest('.spot[data-point]');
  if (spot) { event.preventDefault(); openEditor('spot', spot.dataset.point); }
});
function finishKeyRecording(button) {
  button.setAttribute('aria-pressed', 'false');
  button.textContent = button.form.querySelector('[data-capture-key-value]').value;
}

function saveRecordedKey(button, key) {
  const input = button.form.querySelector('[data-capture-key-value]');
  input.value = key;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  finishKeyRecording(button);
}

// Keep side buttons from invoking browser Back/Forward while using this field.
for (const type of ['mousedown', 'mouseup', 'auxclick']) {
  document.addEventListener(type, function (event) {
    const button = event.target.closest('[data-capture-key]');
    if (!button || ![3, 4].includes(event.button)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    if (type === 'mousedown' && button.getAttribute('aria-pressed') === 'true') {
      if (event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) return;
      saveRecordedKey(button, event.button === 3 ? 'MOUSE4' : 'MOUSE5');
    }
  }, true);
}

document.addEventListener('wheel', function (event) {
  const button = event.target.closest('[data-capture-key][aria-pressed="true"]');
  if (!button || !event.deltaY || event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) return;
  event.preventDefault();
  event.stopImmediatePropagation();
  saveRecordedKey(button, event.deltaY < 0 ? 'MWHEELUP' : 'MWHEELDOWN');
}, { capture: true, passive: false });

function captureKeyFromCode(code) {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1[0-2])$/.test(code)) return code;
  return { Space: 'SPACE', Backspace: 'BACKSPACE', Delete: 'DEL', Insert: 'INS',
    Home: 'HOME', End: 'END', PageUp: 'PGUP', PageDown: 'PGDN' }[code];
}

document.addEventListener('click', function (event) {
  const button = event.target.closest('[data-capture-key]');
  if (!button) return;
  button.setAttribute('aria-pressed', 'true');
  button.textContent = 'Press a key…';
  button.focus();
});

document.addEventListener('focusout', function (event) {
  if (event.target.matches('[data-capture-key]')) finishKeyRecording(event.target);
});

// Capture before global shortcuts and dialog handling so recording never syncs,
// submits the form, or closes the dialog accidentally.
document.addEventListener('keydown', function (event) {
  const button = document.querySelector('[data-capture-key][aria-pressed="true"]');
  if (!button) return;
  if (event.code === 'Tab') { finishKeyRecording(button); return; }
  event.preventDefault();
  event.stopImmediatePropagation();
  if (event.code === 'Escape') { finishKeyRecording(button); return; }
  if (event.repeat || event.isComposing || /^(Control|Shift|Alt|Meta)/.test(event.code)) return;
  const key = captureKeyFromCode(event.code);
  if (event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) {
    notify('Press a single key without modifiers.', true);
    return;
  }
  if (!key) {
    notify('Choose a letter, number, function key, Space, or an editing key. Arrow keys stay reserved for navigation.', true);
    return;
  }
  saveRecordedKey(button, key);
}, true);

document.addEventListener('keydown', function (event) {
  if (spotMenuOpen() && ['Escape', 'Tab'].includes(event.key)) {
    closeSpotMenu(true);
    if (event.key === 'Escape') { event.preventDefault(); return; }
  }
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
    if (mapDrag || spotMenuOpen() || openingEditor || document.querySelector('#editor[open]') || Date.now() - lastSelection < 600) { event.preventDefault(); return; }
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
  if (target.id === 'locations' && (mapDrag || spotMenuOpen() || openingEditor || document.querySelector('#editor[open]') || xhr.lastSelection !== lastSelection)) {
    event.detail.shouldSwap = false; return;
  }
  if (xhr.viewState && (xhr.viewState !== document.getElementById('locations').getAttribute('data-state') || document.querySelector('#editor[open]'))) {
    event.detail.shouldSwap = false; return;
  }
  if (xhr.status >= 400 && xhr.getResponseHeader('HX-Retarget') === '#feedback') event.detail.shouldSwap = true;
  if (target.id === 'locations') {
    const focused = document.activeElement;
    if (focused?.matches('[data-map-viewport]')) xhr.mapFocus = '[data-map-viewport]';
    else if (focused?.matches('[data-map-zoom]')) xhr.mapFocus = `[data-map-zoom="${focused.dataset.mapZoom}"]`;
    else if (focused?.matches('.dot, .marker-cluster')) xhr.mapFocusPoint = focused.dataset.point || focused.dataset.firstPoint;
  }
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
  if (event.detail.target.id === 'locations') {
    initializeMapNavigation();
    if (event.detail.xhr.mapFocus) document.querySelector(event.detail.xhr.mapFocus)?.focus({ preventScroll: true });
    if (event.detail.xhr.mapFocusPoint) document.querySelector('[data-map-viewport]')?.dispatchEvent(new CustomEvent('focusMarker', { detail: event.detail.xhr.mapFocusPoint }));
  }
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
initializeMapNavigation();
