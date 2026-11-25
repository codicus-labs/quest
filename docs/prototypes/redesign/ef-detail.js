/* PROTOTYPE – throwaway. task detail parts and undo.
   - Notes are Markdown: rendered by a small local renderer, E / ↵ / click shows the source, Esc or blur renders again.
   - Checklist per task: text + done, ordered; no dates, quest or time of its own.
   - Tracked time folds to one line; the state holds for the session.
   - Undo (⌘Z, toast): done, delete, date change, quest change. One slot: the most recent of these.
   Loaded after ef-fields.js; reuses the globals of index.html, ef.js, ef-lists.js. */

/* ---------- Demo content ---------- */
const DEMO_NOTES = {
  t1: 'Three screens: **onboarding**, *first list*, first tracked hour.\n\n### Open\n- Empty state: copy or a quiet illustration?\n- Does `Today` start empty?\n\n> Calm first, clever later.\n\nSketches: [shared board](https://example.org/board)',
  t3: 'Topic: why **tracking** beats planning.\n\n1. Hook: a week of plans vs. a week of logs\n2. What the numbers said\n3. One rule to keep\n\n- [x] Pick a title\n- [ ] Find one chart',
  t12: '## Draft\nShort, *one* screenshot, link to the waitlist.',
};
const DEMO_ITEMS = {
  t1: [['Onboarding sketch', true], ['First list', true], ['First tracked hour', false], ['Review with Anna', false], ['Export frames', false]],
  t4: [['Shoes and watch', true], ['Water bottle', false], ['Stretch after', false]],
};
for (const t of T) {
  if (DEMO_NOTES[t.id]) t.notes = DEMO_NOTES[t.id];
  t.items = (DEMO_ITEMS[t.id] || []).map(([text, done]) => ({ id: 'i' + (++seq), text, done }));
}

/* ---------- Markdown → HTML ----------
   Every piece of text goes through esc() before any tag is added; links only for http(s) and mailto.
   Supported: # ## ### headings, - / * / 1. lists, - [ ] checkboxes, > quotes, ``` code, `code`, **bold**, *italic*, [text](url). */
const SAFE_URL = /^(https?:\/\/|mailto:)/i;
function mdInline(raw) {
  const slots = [], slot = html => `\u0000${slots.push(html) - 1}\u0000`;
  let s = esc(raw)
    .replace(/`([^`]+)`/g, (_, code) => slot(`<code>${code}</code>`))
    .replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (all, text, url) => SAFE_URL.test(url) ? slot(`<a href="${url}" target="_blank" rel="noopener noreferrer">${text}</a>`) : text);
  s = s.replace(/\*\*(?=\S)(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/(^|[^*\w])\*(?=\S)(.+?)\*(?!\w)/g, '$1<em>$2</em>')
    .replace(/(^|[^\w])_(?=\S)(.+?)_(?!\w)/g, '$1<em>$2</em>');
  return s.replace(/\u0000(\d+)\u0000/g, (_, i) => slots[i]);
}
/* idp: unique prefix for element ids (focus survives a re-render); taskId: owner of the "- [ ]" lines. */
function md(src, idp, taskId) {
  const out = [];
  let cur = null, fence = null;
  const lines = items => items.map(mdInline).join('<br>');
  const BLOCK = {
    p: items => `<p>${lines(items)}</p>`,
    quote: items => `<blockquote>${lines(items)}</blockquote>`,
    ul: items => `<ul>${items.map(i => `<li>${mdInline(i)}</li>`).join('')}</ul>`,
    ol: items => `<ol>${items.map(i => `<li>${mdInline(i)}</li>`).join('')}</ol>`,
    tasks: items => `<ul class="mdtasks">${items.map(i => `<li class="${i.done ? 'on' : ''}"><button class="mdchk ${i.done ? 'on' : ''}" id="mdc-${idp}-${i.n}" data-a="mdChk" data-id="${taskId}" data-n="${i.n}" role="checkbox" aria-checked="${i.done}" aria-label="${esc(i.text)}">${ic('check')}</button><span>${mdInline(i.text)}</span></li>`).join('')}</ul>`,
  };
  const flush = () => { if (cur) out.push(BLOCK[cur.kind](cur.items)); cur = null; };
  const push = (kind, item) => { if (!cur || cur.kind !== kind) { flush(); cur = { kind, items: [] }; } cur.items.push(item); };
  src.split('\n').forEach((line, n) => {
    if (fence) { if (/^```/.test(line)) { out.push(`<pre><code>${esc(fence.join('\n'))}</code></pre>`); fence = null; } else fence.push(line); return; }
    let m;
    if (/^```/.test(line)) { flush(); fence = []; }
    else if (!line.trim()) flush();
    else if ((m = line.match(/^(#{1,3})\s+(.*)$/))) { flush(); out.push(`<div class="mdh mdh${m[1].length}" role="heading" aria-level="${m[1].length + 2}">${mdInline(m[2])}</div>`); }
    else if ((m = line.match(/^\s*[-*]\s+\[([ xX])\]\s*(.*)$/))) push('tasks', { done: m[1] !== ' ', text: m[2], n });
    else if ((m = line.match(/^\s*[-*+]\s+(.*)$/))) push('ul', m[1]);
    else if ((m = line.match(/^\s*\d+[.)]\s+(.*)$/))) push('ol', m[1]);
    else if ((m = line.match(/^>\s?(.*)$/))) push('quote', m[1]);
    else push('p', line);
  });
  if (fence) out.push(`<pre><code>${esc(fence.join('\n'))}</code></pre>`);
  flush();
  return out.join('');
}

/* ---------- Notes: rendered, or the Markdown source while editing ---------- */
let noteEdit = null; // { id, ctx: 'detail' | 'focus' }
let pendingFocus = null; // { id, caret: number | 'end' | undefined }, applied after the next render
function editNote(id, ctx) {
  noteEdit = { id, ctx };
  pendingFocus = { id: `noted-${ctx}-${id}`, caret: 'end' };
}
function closeNote() {
  const { id, ctx } = noteEdit;
  noteEdit = null;
  pendingFocus = { id: `note-${ctx}-${id}` };
}
function noteBlock(t, ctx) {
  const focusCls = ctx === 'focus' ? 'fnotes' : '';
  if (noteEdit && noteEdit.id === t.id && noteEdit.ctx === ctx) {
    return `<textarea class="mdsrc ${focusCls}" id="noted-${ctx}-${t.id}" data-notes="${t.id}" data-noteed placeholder="Notes · Markdown" aria-label="Notes (Markdown)">${esc(t.notes || '')}</textarea>`;
  }
  return `<div class="mdnote ${focusCls} ${t.notes ? '' : 'blank'}" id="note-${ctx}-${t.id}" tabindex="0" data-a="noteEdit" data-id="${t.id}" data-ctx="${ctx}" title="Edit (E)">${t.notes ? md(t.notes, `${ctx}-${t.id}`, t.id) : 'Notes'}</div>`;
}
function toggleNoteCheckbox(t, n) {
  const lines = t.notes.split('\n');
  lines[n] = lines[n].replace(/\[([ xX])\]/, (_, c) => c === ' ' ? '[x]' : '[ ]');
  t.notes = lines.join('\n');
}

/* ---------- Checklist ---------- */
const newItem = () => ({ id: 'i' + (++seq), text: '', done: false });
const itemProgress = t => t.items && t.items.length ? `<span class="ckprog tnum" title="Checklist: ${t.items.filter(i => i.done).length} of ${t.items.length} done">${t.items.filter(i => i.done).length}/${t.items.length}</span>` : '';
const ckbox = (t, it, idp) => `<button class="ckbox ${it.done ? 'on' : ''}" id="${idp}-${it.id}" data-a="itemToggle" data-id="${t.id}" data-i="${it.id}" role="checkbox" aria-checked="${it.done}" aria-label="${esc(it.text || 'Item')}">${ic('check')}</button>`;
function checklist(t, ctx) {
  const items = t.items || [];
  if (ctx === 'focus') {
    return items.length ? `<div class="cklist fck">${items.map(it => `<div class="ckrow ${it.done ? 'on' : ''}">${ckbox(t, it, 'fck')}<span class="cktext">${esc(it.text)}</span></div>`).join('')}</div>` : '';
  }
  return `<div class="cklist">${items.map(it => `<div class="ckrow ${it.done ? 'on' : ''}">${ckbox(t, it, 'ck')}<input class="cktext" id="it-${it.id}" data-item="${it.id}" data-task="${t.id}" value="${esc(it.text)}" placeholder="Item" autocomplete="off" aria-label="Checklist item"></div>`).join('')}
    <div class="ckrow add"><span class="ckplus">${ic('plus')}</span><input class="cktext" id="itnew-${t.id}" data-itemnew="${t.id}" placeholder="Add checklist item" autocomplete="off"></div></div>`;
}
function moveItem(t, i, dir) {
  const j = i + dir;
  if (j < 0 || j >= t.items.length) return;
  [t.items[i], t.items[j]] = [t.items[j], t.items[i]];
}
/* Keys inside a checklist input; true when handled. */
function itemKey(e, el) {
  const t = task(el.dataset.task || el.dataset.itemnew), k = e.key, cmd = e.metaKey || e.ctrlKey;
  const inputs = [...el.closest('.cklist').querySelectorAll('input.cktext')], at = inputs.indexOf(el);
  const focusInput = (input, caret = 'end') => { if (input) { input.focus(); const c = caret === 'end' ? input.value.length : caret; input.setSelectionRange(c, c); } };
  if (k === 'Escape') { el.blur(); return true; }
  if (!e.altKey && !cmd && (k === 'ArrowUp' || k === 'ArrowDown')) { focusInput(inputs[at + (k === 'ArrowDown' ? 1 : -1)]); return true; }
  if (el.dataset.itemnew) {
    if (k !== 'Enter' || !el.value.trim()) return false;
    t.items.push({ ...newItem(), text: el.value.trim() });
    pendingFocus = { id: el.id };
    return 'render';
  }
  const i = t.items.findIndex(x => x.id === el.dataset.item), it = t.items[i];
  if (k === 'Enter' && cmd) { it.done = !it.done; pendingFocus = { id: el.id, caret: el.selectionStart }; return 'render'; }
  if (k === 'Enter') {
    if (!it.text.trim()) return true;
    const next = newItem();
    t.items.splice(i + 1, 0, next);
    pendingFocus = { id: 'it-' + next.id };
    return 'render';
  }
  if (k === 'Backspace' && !el.value) {
    t.items.splice(i, 1);
    const prev = t.items[i - 1];
    pendingFocus = { id: prev ? 'it-' + prev.id : t.items[i] ? 'it-' + t.items[i].id : 'itnew-' + t.id };
    return 'render';
  }
  if (e.altKey && (k === 'ArrowUp' || k === 'ArrowDown')) {
    moveItem(t, i, k === 'ArrowDown' ? 1 : -1);
    pendingFocus = { id: el.id, caret: el.selectionStart };
    return 'render';
  }
  return false;
}

/* ---------- Tracked time: one line, unfolds on demand ---------- */
let timeOpen = params.get('timeopen') === '1'; // one switch for the session, not per task
function timeBlock(t) {
  const mins = taskMins(t);
  if (!mins) return '';
  return `<div class="tblock"><button class="tsum" id="tsum-${t.id}" data-a="timeToggle" aria-expanded="${timeOpen}" title="${timeOpen ? 'Hide' : 'Show'} time entries">${ic('clock')}<span class="tnum">${fmtDur(mins)}</span>${ic(timeOpen ? 'chev' : 'right')}</button>${timeOpen ? `<div class="tentries">${entryLines(t)}</div>` : ''}</div>`;
}

/* ---------- Undo ---------- */
const LINGER_MS = 4000, UNDO_TOAST_MS = 5000;
let undoSlot = null; // { label, undo }: the most recent undoable change, until another one replaces it
let undoToast = null, undoTimer;
function remember(label, undo, withToast) {
  undoSlot = { label, undo };
  clearTimeout(undoTimer);
  undoToast = withToast ? label : null;
  if (!withToast) return;
  undoTimer = setTimeout(() => { undoToast = null; render(); }, UNDO_TOAST_MS);
}
function forget() { undoSlot = null; undoToast = null; }
function undoLast() {
  if (!undoSlot) { toast('Nothing to undo'); return; }
  const { label, undo } = undoSlot;
  forget();
  undo();
  if (label) toast(`Undone · ${label}`);
}
/* A task ticked off stays visible in place for a moment, then lists that hide done tasks drop it. */
function linger(t) {
  t.lingerUntil = Date.now() + LINGER_MS;
  setTimeout(render, LINGER_MS + 20);
}
const doneState = t => ({ done: t.done, doneToday: t.doneToday, lingerUntil: 0 });
function rememberDone(t, before) {
  linger(t);
  remember('Done', () => Object.assign(t, before), true);
}
const snapshotTask = t => ({ plan: t.plan, due: t.due, quests: [...t.quests], dayOrder: t.dayOrder, sortKey: t.sortKey });
function undoableTaskChange(t, label, apply) {
  const before = snapshotTask(t), postponed = POSTPONES.length;
  apply();
  if (JSON.stringify(before) === JSON.stringify(snapshotTask(t))) return;
  remember(label, () => {
    Object.assign(t, before);
    POSTPONES.length = postponed;
  });
}
function deleteTask(id) {
  const t = task(id);
  if (!t || t.builtin) return;
  const ids = visibleTasks(), i = ids.indexOf(id);
  if (run && run.task === id) stopTimer();
  t.deleted = true; // soft delete, like deleted_at: time entries keep their task
  if (st.sel === id) st.sel = null;
  ef.cur = ids[i + 1] || ids[i - 1] || null;
  remember('Deleted', () => { t.deleted = false; ef.cur = id; }, true);
}

const changeDatesBeforeUndo = changeDates;
changeDates = function (t, next) {
  undoableTaskChange(t, 'Date change', () => changeDatesBeforeUndo(t, next));
};
const applyPickBeforeUndo = applyPick;
applyPick = function (value) {
  const p = ef.pop;
  if (p.kind !== 'quest' || p.target === 'qa') return applyPickBeforeUndo(value);
  undoableTaskChange(task(p.target), 'Quest change', () => applyPickBeforeUndo(value));
};

/* ---------- Actions ---------- */
const toggleBeforeUndo = actions.toggle, focusDoneBeforeUndo = actions.focusDone;
Object.assign(actions, {
  toggle: d => {
    const t = task(d.id), before = doneState(t);
    toggleBeforeUndo(d);
    if (t.done) return rememberDone(t, before);
    t.lingerUntil = 0;
    if (undoSlot && undoSlot.label === 'Done') forget();
  },
  focusDone: () => {
    const t = runningTask(), before = doneState(t);
    focusDoneBeforeUndo();
    toastMsg = ''; // the undo toast says it
    rememberDone(t, before);
  },
  undo: () => undoLast(),
  noteEdit: d => editNote(d.id, d.ctx),
  mdChk: d => { toggleNoteCheckbox(task(d.id), +d.n); },
  itemToggle: d => { const it = task(d.id).items.find(x => x.id === d.i); it.done = !it.done; },
  timeToggle: () => { timeOpen = !timeOpen; },
});

/* Desktop keys outside text fields (called from efKey). True when handled. */
function detailKey(e) {
  const k = e.key, lower = k.length === 1 ? k.toLowerCase() : k, cmd = e.metaKey || e.ctrlKey;
  if (typing(e.target)) return false;
  if (cmd && !e.shiftKey && !e.altKey && lower === 'z') { undoLast(); return true; }
  if (cmd && k === 'Backspace' && ef.cur && !focusOpen()) { deleteTask(ef.cur); return true; }
  if (cmd || e.altKey || e.shiftKey) return false;
  if (e.target.matches && e.target.matches('.mdnote') && (k === 'Enter' || lower === 'e')) { editNote(e.target.dataset.id, e.target.dataset.ctx); return true; }
  if (lower !== 'e') return false;
  if (focusOpen()) { editNote(runningTask().id, 'focus'); return true; }
  if (!ef.cur) return false;
  st.sel = ef.cur;
  editNote(ef.cur, 'detail');
  return true;
}

/* Note source and checklist inputs (desktop and mobile); capture, so Esc does not also leave focus or close the row. */
document.addEventListener('keydown', e => {
  const el = e.target;
  if (!el.dataset) return;
  if (el.dataset.noteed !== undefined && e.key === 'Escape') {
    e.preventDefault(); e.stopPropagation();
    closeNote(); render();
    return;
  }
  if (el.dataset.item === undefined && el.dataset.itemnew === undefined) return;
  const handled = itemKey(e, el);
  if (!handled) return;
  e.preventDefault(); e.stopPropagation();
  if (handled === 'render') render();
}, true);
document.addEventListener('input', e => {
  const el = e.target;
  if (el.dataset.item === undefined) return;
  task(el.dataset.task).items.find(x => x.id === el.dataset.item).text = el.value;
});
/* Blur saves too (the text is already stored on input); a re-render keeps the same id focused, so it is not a blur. */
document.addEventListener('focusout', e => {
  if (!e.target.dataset || e.target.dataset.noteed === undefined) return;
  const id = e.target.id;
  setTimeout(() => {
    if (!noteEdit || (document.activeElement && document.activeElement.id === id)) return;
    noteEdit = null; render();
  });
});
/* Links in a rendered note open; they do not switch the note to editing. */
document.addEventListener('click', e => { if (e.target.closest && e.target.closest('.mdnote a')) e.stopPropagation(); }, true);

/* ---------- Render: undo toast and focus hand-over ---------- */
const renderBeforeDetail = render;
render = function () {
  renderBeforeDetail();
  if (undoToast) {
    const mobile = ui.device === 'mobile', host = mobile ? document.querySelector('.phone .screen') : document.getElementById('app');
    host.insertAdjacentHTML('beforeend', `<div class="undotoast ${mobile ? 'm' : ''}" role="status"><span>${esc(undoToast)}</span><button data-a="undo">Undo</button>${mobile ? '' : '<kbd>⌘Z</kbd>'}</div>`);
  }
  if (!pendingFocus) return;
  const { id, caret } = pendingFocus, el = document.getElementById(id);
  pendingFocus = null;
  if (!el) return;
  el.focus();
  if (caret !== undefined && el.setSelectionRange) { const c = caret === 'end' ? el.value.length : Math.min(caret, el.value.length); el.setSelectionRange(c, c); }
};

/* Screenshot scenes: ?noteedit=t1 opens the source, ?undo=t2 ticks a task off (toast + lingering row), ?undo=del:t17 deletes. */
if (params.get('noteedit')) editNote(params.get('noteedit'), SCENES.includes('focus') ? 'focus' : 'detail');
if (params.get('undo')) {
  const [a, b] = params.get('undo').split(':');
  if (a === 'del') deleteTask(b); else actions.toggle({ id: a });
}
