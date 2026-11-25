/* PROTOTYPE – throwaway. date model (planned / due), lists, Quick-Add with #quest @plan !due.
   Dates are day offsets from today (0 = today, -2 = two days ago); the app would store planned_on / due_on as dates. */

/* ---------- Demo dates: [planned, due] ---------- */
const DEMO_DATES = {
  t1: [0, 2], t2: [0, null], t3: [-1, 0], t4: [0, null], t5: [0, null], t6: [0, null],
  t7: [1, 4], t8: [1, 1], t9: [3, null], t10: [3, null], t11: [null, -1], t12: [-2, 5], t18: [null, -2],
};
for (const [index, t] of T.entries()) {
  const [plan, due] = DEMO_DATES[t.id] || [null, null];
  Object.assign(t, { plan, due, dayOrder: index, sortKey: index });
}

/* ---------- Rule: the plan date never lies after the deadline ----------
   Pure: takes { plan, due }, returns both. The date the user sets wins and pulls the other one along.
   Maps 1:1 to quest-domain (set_planned / set_due on Task, dates instead of day offsets). */
const has = v => v !== null && v !== undefined;
function setPlanned(t, plan) { return { plan, due: has(plan) && has(t.due) && t.due < plan ? plan : t.due }; }
function setDue(t, due) { return { due, plan: has(due) && has(t.plan) && t.plan > due ? due : t.plan }; }

/* ---------- Postponements ----------
   Postponed = a manual change that moves an existing plan date or deadline later; plan and deadline count separately.
   Setting a date for the first time, pulling it earlier or clearing it does not count; a task left lying shows "since …".
   Nothing moves dates on its own: only changeDates (picker, keys, drag) and Quick-Add (new tasks) write them.
   One event per postponed field, like a time entry, so a week can be counted. */
const POSTPONES = [];
function postponedFields(before, after) {
  return ['plan', 'due'].filter(f => has(before[f]) && has(after[f]) && after[f] > before[f]);
}
function changeDates(t, next) {
  for (const field of postponedFields(t, next)) POSTPONES.push({ task: t.id, field, at: now() });
  const dates = { ...t, ...next };
  if (!has(next.dayOrder) && (t.plan !== dates.plan || !isEfToday(t) && isEfToday(dates))) {
    t.dayOrder = endDayOrder(dates, t.id);
  }
  Object.assign(t, next);
}
const postponeCount = (id, field) => POSTPONES.filter(p => p.task === id && (!field || p.field === field)).length;
const weekStart = () => at(-((new Date(base).getDay() + 6) % 7), 0);
const postponedThisWeek = () => POSTPONES.filter(p => p.at >= weekStart()).length;
const DEMO_POSTPONES = [['t1', 'plan', -1], ['t1', 'due', 0], ['t7', 'plan', -6], ['t7', 'plan', -1], ['t12', 'due', -9],
  ['t12', 'due', -4], ['t12', 'due', -1], ['t3', 'due', 0], ['t9', 'plan', -2]];
for (const [id, field, day] of DEMO_POSTPONES) POSTPONES.push({ task: id, field, at: at(day, 10) });
function postMark(id, field) {
  const n = postponeCount(id, field);
  if (!n) return '';
  const tip = field ? `${field === 'plan' ? 'Plan' : 'Deadline'} postponed ${n}×`
    : ['plan', 'due'].map(f => postponeCount(id, f) && `${f === 'plan' ? 'Plan' : 'Deadline'} postponed ${postponeCount(id, f)}×`).filter(Boolean).join(' · ');
  return `<span class="post" title="${tip}">${ic('redo')}${n}</span>`;
}

/* ---------- Day labels ---------- */
const noon = off => at(off, 12);
const wd = off => new Date(noon(off)).toLocaleDateString('en-GB', { weekday: 'short' });
const dnum = off => new Date(noon(off)).getDate();
const dayName = off => off === 0 ? 'Today' : off === 1 ? 'Tomorrow' : off === -1 ? 'Yesterday' : Math.abs(off) < 7 ? wd(off) : new Date(noon(off)).toLocaleDateString('en-GB', { day: 'numeric', month: 'short' });
const pastDay = off => `${wd(off)} ${dnum(off)}`;

/* ---------- Lists ----------
   Deleted tasks are soft-deleted (undo); a task just ticked off still looks open for a moment (ef-detail.js). */
const liveT = () => T.filter(t => !t.deleted);
const isLingering = t => !!t.lingerUntil && Date.now() < t.lingerUntil;
const looksOpen = t => !t.done || isLingering(t);
const isEfInbox = t => looksOpen(t) && !t.quests.length && !has(t.plan);
const isEfToday = t => (has(t.plan) && t.plan <= 0) || (has(t.due) && t.due <= 0);
const EF_LISTS = {
  inbox: () => liveT().filter(isEfInbox),
  today: () => liveT().filter(t => t.done ? t.doneToday || t.plan === 0 : isEfToday(t)),
  plan: () => liveT().filter(t => looksOpen(t) && has(t.plan) && t.plan > 0),
  logbook: () => liveT().filter(t => t.done),
};
const efListOf = v => v.startsWith('q:') ? liveT().filter(t => t.quests.includes(v.slice(2))) : EF_LISTS[v]();

const EF_NAV = [['inbox', 'Inbox', 'inbox'], ['today', 'Today', 'sun'], ['plan', 'Plan', 'cal'], ['logbook', 'Logbook', 'book']];
function efNav() {
  return EF_NAV.map(([v, label, i]) => {
    const n = v === 'logbook' ? '' : efListOf(v).filter(t => !t.done).length || '';
    return `<button class="nav ${v} ${st.view === v ? 'on' : ''}" data-a="nav" data-v="${v}" ${v === 'today' || v === 'inbox' ? `data-file="${v}"` : ''}>${ic(i)}<span>${label}</span><span class="n">${n}</span></button>`;
  }).join('');
}
function efHead() {
  const v = st.view;
  if (v.startsWith('q:')) {
    const q = quest(v.slice(2)), m = questMins(q.id);
    return `<h1>${ring(m / 60 / q.goal, q.color, 28, 3.5)}${esc(q.title)}</h1><div class="sub">${fmtH(m)} of ${q.goal}h this week · ${efListOf(v).filter(t => !t.done).length} open</div>`;
  }
  const [, label, i] = EF_NAV.find(([k]) => k === v);
  const sub = v === 'today' ? `${dateLabel} · <span class="tnum" data-today></span> tracked` : '';
  return `<h1 class="${v}">${ic(i)}${label}</h1><div class="sub">${sub}</div>`;
}

/* ---------- Rows ----------
   The row keeps what decides the next step: dates, checklist progress, quest. Notes and postponements live in the detail.
   ?dueflag=0 drops the flag before "due …"; ?rowtime=0 drops the tracked duration (a running timer still shows). */
const DUE_FLAG = params.get('dueflag') !== '0', ROW_TIME = params.get('rowtime') !== '0';
function dateMarks(t, inPlan) {
  const marks = [];
  if (!t.done && has(t.plan) && t.plan < 0) marks.push(`<span class="dmark">since ${pastDay(t.plan)}</span>`);
  if (!t.done && has(t.plan) && t.plan > 0 && !inPlan) marks.push(`<span class="dmark">${ic('cal')}${dayName(t.plan)}</span>`);
  if (!t.done && has(t.due)) {
    const cls = t.due < 0 ? 'over' : t.due === 0 ? 'today' : '';
    const label = t.due < 0 ? `overdue · ${pastDay(t.due)}` : t.due === 0 ? 'due today' : `due ${dayName(t.due)}`;
    marks.push(`<span class="dmark ${cls}">${DUE_FLAG ? ic('flag') : ''}${label}</span>`);
  }
  return marks.join('');
}
function efRow(t, sortable = false) {
  const q = primary(t), isOpen = st.sel === t.id, mins = taskMins(t), mobile = ui.device === 'mobile';
  const showQuest = q && !st.view.startsWith('q:'), marks = dateMarks(t, st.view === 'plan') + itemProgress(t);
  return `<div class="item ${isOpen ? 'open' : ''} ${t.done ? 'done' : ''} ${ef.cur === t.id ? 'kb' : ''}" data-tid="${t.id}">
    <div class="line"${sortable && !t.done ? ` data-drag="${t.id}"${mobile ? '' : ' draggable="true"'}` : ''} data-a="sel" data-id="${t.id}">${chk(t)}
      <span class="title">${esc(t.title)}${mobile && marks ? `<span class="msub">${marks}</span>` : ''}</span>
      <span class="meta">${mobile ? '' : marks}${showQuest ? `<span class="row" style="gap:5px"><span class="dot" style="--c:${q.color}"></span>${mobile ? '' : esc(q.title)}</span>` : ''}${ROW_TIME && mins && !(run && run.task === t.id) && !mobile ? `<span class="tnum">${fmtDur(mins)}</span>` : ''}</span>
      ${playBtn(t)}</div>
    ${isOpen ? `<div class="detail">
      ${noteBlock(t, 'detail')}${checklist(t, 'detail')}
      ${efFields(t.id)}
      ${timeBlock(t)}
    </div>` : ''}</div>`;
}
function efList() {
  const v = st.view;
  if (v === 'plan') return planList();
  const items = efListOf(v);
  if (v === 'logbook') return items.map(t => efRow(t)).join('');
  if (!items.length) return `<div class="empty">${v === 'inbox' ? 'Inbox zero' : 'Nothing here.'}</div>`;
  return taskListRows(v, items);
}
function taskListRows(group, items) {
  const openItems = items.filter(looksOpen), done = items.filter(t => !looksOpen(t));
  return sortGroup(group, openItems) + (done.length ? `<div class="grp faint" style="font-weight:500">${done.length} done</div>${done.map(t => efRow(t)).join('')}` : '');
}

/* ---------- Quick-Add: "#quest" files it, "@fri" plans it, "!fri" sets the deadline ----------
   Focus unfolds the same field row as the task detail; the tokens fill it. Its defaults show what ↵ will do. */
const qa = { text: params.get('qa') || '', caret: null, idx: 0, quest: null, plan: null, due: null, view: undefined, focused: params.get('qaopen') === '1' };
const qaDefaults = view => ({ quest: view.startsWith('q:') ? view.slice(2) : null, plan: view.startsWith('plan:') ? +view.slice(5) : view === 'today' ? 0 : null, due: null });
function resetQa(view) { Object.assign(qa, { text: '', caret: 0, idx: 0, view }, qaDefaults(view)); }
const qaOpen = () => qa.view === 'global' || qa.focused || !!qa.text || !!(ef.pop && ef.pop.target === 'qa');
const WD2 = ['su', 'mo', 'tu', 'we', 'th', 'fr', 'sa'];
function resolveDay(word) {
  const w = word.toLowerCase();
  if (w === 'today' || w === 'tod') return 0;
  if (w.startsWith('tom')) return 1;
  const i = WD2.indexOf(w.slice(0, 2));
  return w.length < 2 || i < 0 ? null : (i - new Date(base).getDay() + 7) % 7;
}
function acToken() {
  const caret = qa.caret ?? qa.text.length, m = qa.text.slice(0, caret).match(/(?:^|\s)([#@!])(\w*)$/);
  return m ? { sigil: m[1], frag: m[2].toLowerCase() } : null;
}
function acItems() {
  const tok = acToken();
  if (!tok) return [];
  if (tok.sigil === '#') return Q.filter(q => q.tag.startsWith(tok.frag) || q.title.toLowerCase().split(/\s+/).some(w => w.startsWith(tok.frag))).map(q => ({ kind: 'quest', id: q.id }));
  const field = tok.sigil === '@' ? 'plan' : 'due';
  return WEEK.filter(o => !tok.frag || dayName(o).toLowerCase().startsWith(tok.frag) || wd(o).toLowerCase().startsWith(tok.frag)).map(o => ({ kind: field, id: o }));
}
function pickAc(item) {
  const caret = qa.caret ?? qa.text.length;
  const before = qa.text.slice(0, caret).replace(/(^|\s)[#@!]\w*$/, '$1');
  qa.text = before + qa.text.slice(caret);
  qa.caret = before.length;
  if (item.kind === 'quest') qa.quest = item.id;
  else Object.assign(qa, item.kind === 'plan' ? setPlanned(qa, item.id) : setDue(qa, item.id));
  qa.idx = 0;
}
function efQuickAddFor(view) {
  if (['logbook', 'plan'].includes(view) || (ui.device === 'mobile' && view === 'quests')) return '';
  if (qa.view !== view) { const draft = qa.view === undefined ? qa.text : ''; resetQa(view); qa.text = draft; } // ?qa= seeds the first draft
  const items = ef.pop ? [] : acItems(), open = qaOpen();
  const menu = items.length ? `<div class="acmenu">${items.map((it, i) => `<button class="${i === qa.idx ? 'on' : ''}" data-a="qapick" data-k="${it.kind}" data-q="${it.id}">${it.kind === 'quest'
    ? `<span class="dot" style="--c:${quest(it.id).color}"></span><span class="grow">${esc(quest(it.id).title)}</span><span class="faint">#${quest(it.id).tag}</span>`
    : `${ic(it.kind === 'plan' ? 'cal' : 'flag', 'faint')}<span class="grow">${it.kind === 'plan' ? 'Plan for' : 'Due'} ${dayName(it.id)}</span><span class="faint">${wd(it.id)} ${dnum(it.id)}</span>`}</button>`).join('')}</div>` : '';
  const ph = view === 'inbox' || view === 'global' ? 'Throw a to-do in…' : 'New to-do…';
  return `<div class="qadd efqa ${open ? 'open' : ''}"><div class="qline">${view === 'global' ? logo() : ic('plus')}<input id="qa-ef" data-efadd="${view}" value="${esc(qa.text)}" placeholder="${ph}" autocomplete="off">${ui.device === 'mobile' ? '' : '<kbd>↵</kbd>'}${menu}</div>${open ? efFields('qa') : ''}</div>`;
}
/* Creates the task and resets the Quick-Add for the next one; returns the new task (or nothing for an empty title). */
function efAddTask(view) {
  // Leftover tokens typed without picking from the menu still count, e.g. "Essay @tom !fri".
  const title = qa.text.replace(/(^|\s)([@!])(\w+)(?=\s|$)/g, (all, pre, sigil, word) => {
    const off = resolveDay(word);
    if (off === null) return all;
    Object.assign(qa, sigil === '@' ? setPlanned(qa, off) : setDue(qa, off));
    return pre;
  }).replace(/\s+/g, ' ').trim();
  if (!title) return null;
  const quests = qa.quest ? [qa.quest] : [], { plan, due } = qa;
  const t = { id: 't' + (++seq), title, quests, plan, due };
  t.dayOrder = endDayOrder(t);
  t.sortKey = endSortKey(quests[0]);
  T.unshift(t);
  if (view !== 'global') {
    const where = [quests.length && quest(quests[0]).title, has(plan) && `planned ${dayName(plan)}`, has(due) && `due ${dayName(due)}`].filter(Boolean);
    toast(where.length ? `Added · ${where.join(' · ')}` : 'Added to Inbox');
  }
  resetQa(view);
  return t;
}

Object.assign(actions, {
  qapick: d => { pickAc({ kind: d.k, id: d.k === 'quest' ? d.q : +d.q }); focusQa(); },
});
let refocusQa = false;
function focusQa() { refocusQa = true; }

/* render() rebuilds the DOM; keep the quick-add caret where the user left it. */
let qaRendering = false;
const baseRender = render;
render = function () {
  const hadFocus = document.activeElement && document.activeElement.id === 'qa-ef';
  qaRendering = true;
  baseRender();
  qaRendering = false;
  const el = document.getElementById('qa-ef');
  if (!el || !(hadFocus || refocusQa)) return;
  refocusQa = false;
  el.focus();
  const c = Math.min(qa.caret ?? el.value.length, el.value.length);
  el.setSelectionRange(c, c);
};

/* Unfold on focus; fold (and drop the picks) when focus leaves an empty Quick-Add. */
document.addEventListener('focusin', e => {
  if (qaRendering || qa.focused || !e.target.closest('.efqa')) return;
  qa.focused = true; render();
});
document.addEventListener('focusout', e => {
  if (qaRendering || !e.target.closest('.efqa')) return;
  if (e.relatedTarget && e.relatedTarget.closest('.efqa, .efpopsheet')) return;
  qa.focused = false;
  if (!qa.text && !(ef.pop && ef.pop.target === 'qa')) resetQa(qa.view);
  setTimeout(render);
});
document.addEventListener('mousedown', e => { if (e.target.closest('.acmenu')) e.preventDefault(); });
if (qa.focused) focusQa(); // ?qaopen=1: screenshot scene with the Quick-Add focused
document.addEventListener('input', e => {
  const el = e.target;
  if (el.dataset.efadd === undefined) return;
  const hadMenu = acItems().length, hadText = !!qa.text;
  qa.text = el.value; qa.caret = el.selectionStart; qa.idx = 0;
  if (hadMenu || acItems().length || hadText !== !!qa.text) render();
});
document.addEventListener('selectionchange', () => {
  const el = document.activeElement;
  if (el && el.id === 'qa-ef') qa.caret = el.selectionStart;
});
document.addEventListener('keydown', e => {
  const el = e.target;
  if (!el.dataset || el.dataset.efadd === undefined) return;
  const items = acItems();
  if (items.length && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
    e.preventDefault(); qa.idx = (qa.idx + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length; render(); return;
  }
  if (items.length && (e.key === 'Enter' || e.key === 'Tab')) { e.preventDefault(); pickAc(items[qa.idx]); render(); return; }
  if (e.key === 'Escape' && items.length) { e.stopPropagation(); qa.text = qa.text.replace(/(^|\s)[#@!]\w*$/, '$1'); qa.caret = qa.text.length; render(); return; }
  if (e.key === 'Enter') {
    e.preventDefault();
    const added = efAddTask(el.dataset.efadd);
    if (added && el.dataset.efadd === 'global') globalAdded(added);
    render(); return;
  }
}, true);

