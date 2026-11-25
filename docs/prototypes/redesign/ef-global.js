/* PROTOTYPE – throwaway. Variant F, desktop: a fake macOS desktop (?scene=global) with Quest's global Quick-Add panel
   (⌃⌥Space, simulated in the page) and the floating timer widget. Everything inside reuses the app's Quick-Add,
   field row, pickers and timer state; only the frame is new. */

Object.assign(P, {
  pause: '<rect x="7" y="6" width="3.5" height="12" rx="1" fill="currentColor" stroke="none"/><rect x="13.5" y="6" width="3.5" height="12" rx="1" fill="currentColor" stroke="none"/>',
});

const glob = {
  on: SCENES.includes('global'),
  panel: params.get('panel') === '1', // the floating Quick-Add window
  landed: null, // short feedback after ↵: [icon html, label]
  widget: ['card', 'pill', 'both'].includes(params.get('widget')) ? params.get('widget') : 'card',
  paused: null, // { task, mins } after Pause; Stop hides the widget
  pos: {}, // dragged widget positions: { card: { x, y }, pill: { x, y } }
};
const inScene = () => glob.on && ui.device === 'desktop' && variantKey() === 'F';

/* ---------- Where a new task shows up ---------- */
function landing(t) {
  if (isEfToday(t)) return [ic('sun'), 'Today'];
  if (has(t.plan)) return [ic('cal'), `Plan · ${wd(t.plan)} ${dnum(t.plan)}`];
  const q = primary(t);
  if (q) return [`<span class="dot" style="--c:${q.color}"></span>`, esc(q.title)];
  return [ic('inbox'), 'Inbox'];
}

/* ---------- Panel ---------- */
function openPanel() { Object.assign(glob, { panel: true, landed: null }); qa.view = null; focusQa(); }
function closePanel() { glob.panel = false; ef.pop = null; Object.assign(qa, { view: null, focused: false }); }
let landedTimer;
function globalAdded(t) {
  closePanel();
  glob.landed = landing(t);
  clearTimeout(landedTimer);
  landedTimer = setTimeout(() => { glob.landed = null; render(); }, 1800);
}

/* ---------- Widget ---------- */
function widgetBody(size) {
  if (run && run.task === ORG.id) {
    const from = run.meta && task(run.meta.from);
    const back = from ? `<button class="gw-btn" data-a="play" data-id="${from.id}" title="Back to “${esc(from.title)}”" aria-label="Back to ${esc(from.title)}">${ic('undo')}</button>` : '';
    if (size === 'pill') return `<button class="gw-main" data-a="gwOpen" title="Organizing · open Quest"><span class="dot pulse" style="--c:var(--faint)"></span><span class="gw-time tnum" data-elapsed></span></button>${back}`;
    return `<button class="gw-main" data-a="gwOpen" title="Open Quest"><span class="dot pulse" style="--c:var(--faint)"></span><span class="gw-title ell">Organizing</span></button>
      <div class="gw-row"><span class="gw-time tnum" data-elapsed></span><span class="grow"></span><button class="gw-btn" data-a="gwStop" aria-label="Stop">${ic('stop')}</button></div>
      ${from ? `<button class="backto" data-a="play" data-id="${from.id}">${ic('undo')}<span class="ell">Back to “${esc(from.title)}”</span></button>` : ''}`;
  }
  const t = run ? task(run.task) : task(glob.paused.task), q = primary(t), color = q ? q.color : 'var(--acc)';
  const time = run ? `<span class="gw-time tnum" data-elapsed></span>` : `<span class="gw-time tnum paused">${fmtClock(glob.paused.mins * 60e3)}</span>`;
  const toggle = run ? `<button class="gw-btn" data-a="gwPause" aria-label="Pause">${ic('pause')}</button>` : `<button class="gw-btn" data-a="gwResume" aria-label="Resume">${ic('play')}</button>`;
  if (size === 'pill') return `<button class="gw-main" data-a="gwOpen" title="${esc(t.title)} · open in focus"><span class="dot ${run ? 'pulse' : ''}" style="--c:${color}"></span>${time}</button>`;
  return `<button class="gw-main" data-a="gwOpen" title="Open in focus"><span class="dot ${run ? 'pulse' : ''}" style="--c:${color}"></span><span class="gw-title ell">${esc(t.title)}</span></button>
    <div class="gw-row">${time}<span class="grow"></span>${toggle}<button class="gw-btn" data-a="gwStop" aria-label="Stop">${ic('stop')}</button></div>`;
}
function widget(size, defaultTop) {
  if (!run && !glob.paused) return '';
  const p = glob.pos[size], place = p ? `left:${p.x}px;top:${p.y}px` : `right:28px;top:${defaultTop}px`;
  return `<div class="gwidget gw-${size}" data-gw="${size}" style="${place}" role="group" aria-label="Quest timer">${widgetBody(size)}</div>`;
}

/* ---------- Scene ---------- */
function fakeWindow() {
  const line = w => `<i style="width:${w}%"></i>`;
  return `<div class="fwin" data-a="gBlur"><div class="fwbar"><span></span><span></span><span></span><b>Draft</b></div>
    <div class="fwbody">${[92, 85, 97, 60, 0, 88, 94, 79, 0, 90, 70].map(w => w ? line(w) : '<br>').join('')}</div></div>`;
}
function globalDesk() {
  const sizes = glob.widget === 'both' ? [['card', 46], ['pill', 178]] : [[glob.widget, 46]];
  const clock = new Date(now()).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' });
  return `<div class="fdesk">
    <div class="fwall" data-a="gBlur"></div>
    <div class="fmenu"><b>Draft</b><span>File</span><span>Edit</span><span>View</span><span>Window</span><span class="grow"></span><span>${wd(0)} ${dnum(0)} ${clock}</span></div>
    ${fakeWindow()}
    ${sizes.map(([s, top]) => widget(s, top)).join('')}
    ${glob.panel ? `<div class="gpanel" role="dialog" aria-label="Quest · New to-do">${efQuickAddFor('global')}</div>` : ''}
    ${glob.landed ? `<div class="glanded" role="status">${ic('check')}${glob.landed[0]}<span>${glob.landed[1]}</span></div>` : ''}
  </div>`;
}
const deskBeforeGlobal = VIEWS.F[0];
VIEWS.F[0] = () => inScene() ? globalDesk() : deskBeforeGlobal();

/* ---------- Actions ---------- */
function leaveScene() { closePanel(); glob.on = false; syncUrl(); }
Object.assign(actions, {
  gBlur: () => { if (glob.panel) closePanel(); },
  gwOpen: () => { leaveScene(); if (runningTask()) openFocus(); },
  gwPause: () => { const id = run.task, mins = runMins(); stopTimer(); toastMsg = ''; glob.paused = { task: id, mins }; },
  gwResume: () => { const id = glob.paused.task; glob.paused = null; basePlay({ id }); },
  gwStop: () => { stopTimer(); toastMsg = ''; glob.paused = null; },
});

/* Keys in the scene: the simulated global hotkey, Esc closes the panel. Called from efKey (ef-keys.js). */
function globalKey(e) {
  if (e.ctrlKey && e.altKey && e.code === 'Space') { if (glob.panel) closePanel(); else openPanel(); return true; }
  if (e.key === 'Escape' && glob.panel && !acItems().length) { closePanel(); return true; }
  return false;
}

/* ---------- Drag the widget (pointer); a drag does not count as a click ---------- */
let gwDrag = null, gwDragged = false;
document.addEventListener('pointerdown', e => {
  const w = e.target.closest && e.target.closest('[data-gw]');
  if (!w || e.button !== 0) return;
  const r = w.getBoundingClientRect();
  gwDrag = { el: w, size: w.dataset.gw, dx: e.clientX - r.left, dy: e.clientY - r.top, x0: e.clientX, y0: e.clientY };
  gwDragged = false;
});
document.addEventListener('pointermove', e => {
  if (!gwDrag) return;
  if (!gwDragged && Math.hypot(e.clientX - gwDrag.x0, e.clientY - gwDrag.y0) < 4) return;
  gwDragged = true;
  const x = Math.max(0, Math.min(innerWidth - gwDrag.el.offsetWidth, e.clientX - gwDrag.dx));
  const y = Math.max(28, Math.min(innerHeight - gwDrag.el.offsetHeight, e.clientY - gwDrag.dy));
  glob.pos[gwDrag.size] = { x, y };
  Object.assign(gwDrag.el.style, { left: x + 'px', top: y + 'px', right: 'auto' });
});
document.addEventListener('pointerup', () => { gwDrag = null; });
document.addEventListener('click', e => {
  if (!gwDragged) return;
  gwDragged = false;
  if (e.target.closest('[data-gw]')) { e.stopPropagation(); e.preventDefault(); }
}, true);

/* ---------- Prototype pill: scene toggle, simulated hotkey, widget size; keep ?scene=global in the URL ---------- */
const syncUrlBeforeGlobal = syncUrl;
syncUrl = function () {
  syncUrlBeforeGlobal();
  if (glob.on) history.replaceState(null, '', location.search + '&scene=global');
};
const renderPillBeforeGlobal = renderPill;
renderPill = function () {
  renderPillBeforeGlobal();
  if (variantKey() !== 'F' || ui.device !== 'desktop') return;
  const extra = glob.on
    ? `<button data-p="gscene">App</button><button data-p="ghot" title="Simulated global hotkey">⌃⌥Space</button><button data-p="gsize" title="Widget size">Widget: ${glob.widget}</button>`
    : '<button data-p="gscene" title="Fake macOS desktop: global Quick-Add and timer widget">Mac</button>';
  document.querySelector('#pill .pill').insertAdjacentHTML('beforeend', `<span class="sep"></span>${extra}`);
};
document.getElementById('pill').addEventListener('click', e => {
  const b = e.target.closest('[data-p]');
  if (!b || !b.dataset.p.startsWith('g')) return;
  if (b.dataset.p === 'gscene') { if (glob.on) leaveScene(); else { glob.on = true; syncUrl(); } }
  if (b.dataset.p === 'ghot') { if (glob.panel) closePanel(); else openPanel(); }
  if (b.dataset.p === 'gsize') glob.widget = { card: 'pill', pill: 'both', both: 'card' }[glob.widget];
  render();
});
if (glob.panel) focusQa();
