/* PROTOTYPE – throwaway. Variants E and F: desktop keyboard control and the shortcut overview ("?").
   Single keys only act when no text field has focus; chords avoid macOS and browser reservations (⌘W, ⌘Q, ⌘C, …). */

const VIEW_KEYS = ['inbox', 'today', 'plan', 'logbook', ...Q.map(q => 'q:' + q.id)]; // ⌘1 … ⌘8
const FIELD_CHORDS = { l: 'quest', p: 'plan', d: 'due' }; // ⌘⇧L / ⌘⇧P / ⌘⇧D in the Quick-Add field
const KEY_TABLE = [
  ['Lists', [['N', 'New to-do (Quick-Add)'], ['⌘1 … ⌘4', 'Inbox · Today · Plan · Logbook'], ['⌘5 … ⌘8', 'Quests'], ['T · ⌥⌘I', 'Tracked column'], ['?', 'Shortcuts']]],
  ['Selected to-do', [['↑ ↓ · J K', 'Select'], ['↵', 'Open / close'], ['Space', 'Start / stop timer'], ['F', 'Focus'], ['X', 'Done'], ['L', 'Quest'], ['P', 'Plan'], ['D', 'Deadline'], ['Esc', 'Close / deselect']]],
  ['Quick-Add', [['⌘⇧L · L', 'Quest'], ['⌘⇧P · P', 'Plan'], ['⌘⇧D · D', 'Deadline'], ['#  @  !', 'Quest · plan · deadline inline']]],
  ['Calendar', [['← → ↑ ↓', 'Day · week'], ['PgUp PgDn', 'Month'], ['T M N', 'Today · tomorrow · next Monday'], ['⌫', 'None'], ['↵', 'Set'], ['Esc', 'Cancel']]],
  ['Timer', [['O', 'Organize'], ['B', 'Book on Organizing (hint)'], ['Esc', 'Leave focus']]],
];
function efKeys() {
  return `<div class="efbg" style="position:fixed" data-a="keys"></div><div class="efkeys" role="dialog" aria-label="Shortcuts">
    ${KEY_TABLE.map(([group, rows]) => `<div><div class="lbl2">${group}</div><table>${rows.map(([k, a]) => `<tr><td><kbd>${k}</kbd></td><td>${a}</td></tr>`).join('')}</table></div>`).join('')}</div>`;
}

const visibleTasks = () => [...document.querySelectorAll('.vEF [data-tid]')].map(el => el.dataset.tid);
function moveSelection(dir) {
  const ids = visibleTasks();
  if (!ids.length) return;
  const i = ids.indexOf(ef.cur);
  ef.cur = ids[i < 0 ? (dir > 0 ? 0 : ids.length - 1) : Math.min(ids.length - 1, Math.max(0, i + dir))];
  if (st.sel && st.sel !== ef.cur) st.sel = null;
}
function goView(v) { actions.nav({ v }); ef.cur = null; }
function focusQuickAdd() {
  if (!document.getElementById('qa-ef')) goView('inbox');
  focusQa();
}
const typing = el => el && el.matches && el.matches('input, textarea, [contenteditable]');

/* Returns true when the key was handled. */
function efKey(e) {
  const k = e.key, lower = k.length === 1 ? k.toLowerCase() : k, cmd = e.metaKey || e.ctrlKey;
  if (ef.keys) { if (k === 'Escape' || k === '?') ef.keys = false; return k === 'Escape' || k === '?'; }
  if (ef.pop) return popKey(e);

  // Chords that also work inside the Quick-Add field.
  if (cmd && e.shiftKey && FIELD_CHORDS[lower] && e.target.id === 'qa-ef') { openPop('qa', FIELD_CHORDS[lower], 'qa'); return true; }
  if (inScene()) return globalKey(e); // fake macOS desktop (ef-global.js): only the global hotkey and Esc
  if (cmd && !e.shiftKey && !e.altKey && /^[1-9]$/.test(k) && VIEW_KEYS[k - 1]) { goView(VIEW_KEYS[k - 1]); return true; }
  // ⌥⌘I is the macOS inspector key; the browser keeps it for DevTools, so the prototype also takes ⌃⌥I.
  if (cmd && e.altKey && !e.shiftKey && e.code === 'KeyI') { ef.right = !ef.right; return true; }
  if (typing(e.target) || cmd || e.altKey) return false;

  // Focus view: only its own keys.
  if (focusOpen()) {
    if (k === 'Escape' && !strict()) { leaveFocus(); return true; }
    if (k === ' ') { actions.focusStop(); return true; }
    if (lower === 'x') { actions.focusDone(); return true; }
    if (lower === 'o') { actions.organize(); return true; }
    if (k === '?') { ef.keys = true; return true; }
    return false;
  }
  // On the Quick-Add's field row (Tab from the text) the list letters work too; in the text they would type.
  if (FIELD_CHORDS[lower] && e.target.closest('.efqa')) { openPop('qa', FIELD_CHORDS[lower], 'qa'); return true; }
  const onButton = e.target.matches && e.target.matches('button');
  const t = ef.cur && task(ef.cur);
  switch (lower) {
    case '?': ef.keys = true; return true;
    case 'n': focusQuickAdd(); return true;
    case 't': ef.right = !ef.right; return true;
    case 'o': if (!runningTask()) return false; actions.organize(); return true;
    case 'b': if (!hintDue()) return false; actions.rebook(); return true;
    case 'j': case 'ArrowDown': moveSelection(1); return true;
    case 'k': case 'ArrowUp': moveSelection(-1); return true;
    case 'Escape': if (st.sel) st.sel = null; else ef.cur = null; return true;
  }
  if (!t) return false;
  switch (lower) {
    case 'Enter': if (onButton || st.view === 'plan') return false; st.sel = st.sel === t.id ? null : t.id; return true;
    case ' ': if (onButton || t.done) return false; actions.play({ id: t.id }); return true;
    case 'f': if (t.done) return false; if (!(run && run.task === t.id)) actions.play({ id: t.id }); openFocus(); return true;
    case 'x': actions.toggle({ id: t.id }); return true;
    case 'l': case 'p': case 'd': openPop(t.id, FIELD_CHORDS[lower], st.view === 'plan' ? null : `fld-${FIELD_CHORDS[lower]}-${t.id}`); return true;
  }
  return false;
}
addEventListener('keydown', e => {
  if (!isEF() || ui.device !== 'desktop') return;
  if (!efKey(e)) return;
  e.preventDefault(); e.stopPropagation();
  render();
}, true);

/* A click on a row also moves the keyboard selection there. */
document.addEventListener('click', e => {
  const row = isEF() && e.target.closest && e.target.closest('[data-tid]');
  if (row) ef.cur = row.dataset.tid;
}, true);
Object.assign(actions, { keys: () => { ef.keys = !ef.keys; } });
