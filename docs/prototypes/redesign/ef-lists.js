/* PROTOTYPE – throwaway. Variants E and F: date model (planned / due), lists, Plan week, Quick-Add with #quest @plan !due.
   Dates are day offsets from today (0 = today, -2 = two days ago); the app would store planned_on / due_on as dates. */

/* ---------- Demo dates: [planned, due] ---------- */
const DEMO_DATES = {
  t1: [0, 2], t2: [0, null], t3: [-1, 0], t4: [0, null], t5: [0, null], t6: [0, null],
  t7: [1, 4], t8: [1, 1], t9: [3, null], t10: [3, null], t11: [null, -1], t12: [-2, 5], t18: [null, -2],
};
for (const t of T) { const [plan, due] = DEMO_DATES[t.id] || [null, null]; t.plan = plan; t.due = due; }

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
const WEEK = [0, 1, 2, 3, 4, 5, 6];
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
const todayRank = t => has(t.due) && t.due < 0 ? 0 : has(t.plan) && t.plan < 0 ? 1 : has(t.due) && t.due === 0 ? 2 : 3;
const EF_LISTS = {
  inbox: () => liveT().filter(isEfInbox),
  today: () => liveT().filter(t => t.done ? t.doneToday || t.plan === 0 : isEfToday(t)).sort((a, b) => todayRank(a) - todayRank(b)),
  plan: () => liveT().filter(t => looksOpen(t) && has(t.plan) && t.plan > 0),
  logbook: () => liveT().filter(t => t.done),
};
const efListOf = v => v.startsWith('q:') ? liveT().filter(t => t.quests.includes(v.slice(2))) : EF_LISTS[v]();

const EF_NAV = [['inbox', 'Inbox', 'inbox'], ['today', 'Today', 'sun'], ['plan', 'Plan', 'cal'], ['logbook', 'Logbook', 'book']];
function efNav() {
  return EF_NAV.map(([v, label, i]) => {
    const n = v === 'logbook' ? '' : efListOf(v).filter(t => !t.done).length || '';
    return `<button class="nav ${v} ${st.view === v ? 'on' : ''}" data-a="nav" data-v="${v}">${ic(i)}<span>${label}</span><span class="n">${n}</span></button>`;
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
function dateMarks(t, inPlanCol) {
  const marks = [];
  if (!t.done && has(t.plan) && t.plan < 0) marks.push(`<span class="dmark">since ${pastDay(t.plan)}</span>`);
  if (!t.done && has(t.plan) && t.plan > 0 && !inPlanCol) marks.push(`<span class="dmark">${ic('cal')}${dayName(t.plan)}</span>`);
  if (!t.done && has(t.due)) {
    const cls = t.due < 0 ? 'over' : t.due === 0 ? 'today' : '';
    const label = t.due < 0 ? `overdue · ${pastDay(t.due)}` : t.due === 0 ? 'due today' : `due ${dayName(t.due)}`;
    marks.push(`<span class="dmark ${cls}">${DUE_FLAG ? ic('flag') : ''}${label}</span>`);
  }
  return marks.join('');
}
function efRow(t) {
  const q = primary(t), isOpen = st.sel === t.id, mins = taskMins(t), mobile = ui.device === 'mobile';
  const showQuest = q && !st.view.startsWith('q:'), marks = dateMarks(t, st.view === 'plan') + itemProgress(t);
  return `<div class="item ${isOpen ? 'open' : ''} ${t.done ? 'done' : ''} ${ef.cur === t.id ? 'kb' : ''}" data-tid="${t.id}">
    <div class="line" data-a="sel" data-id="${t.id}">${chk(t)}
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
  if (v === 'plan') return ui.device === 'mobile' ? planMobile() : planDesk();
  const items = efListOf(v);
  if (v === 'logbook') return items.map(efRow).join('');
  if (!items.length) return `<div class="empty">${v === 'inbox' ? 'Inbox zero' : 'Nothing here.'}</div>`;
  const openItems = items.filter(looksOpen), done = items.filter(t => !looksOpen(t));
  return openItems.map(efRow).join('') + (done.length ? `<div class="grp faint" style="font-weight:500">${done.length} done</div>${done.map(efRow).join('')}` : '');
}

/* ---------- Plan (next seven days) ---------- */
const openTasks = () => liveT().filter(looksOpen);
const plannedOn = off => openTasks().filter(t => has(t.plan) && (off === 0 ? t.plan <= 0 : t.plan === off));
const dueOn = off => openTasks().filter(t => has(t.due) && (off === 0 ? t.due <= 0 : t.due === off));
const unplanned = () => openTasks().filter(t => !has(t.plan));
function planCard(t) {
  const q = primary(t), marks = dateMarks(t, true);
  const pop = ef.pop && ef.pop.target === t.id && ui.device !== 'mobile' ? efPopover(has(t.plan) && t.plan >= 4 ? 'right:0' : 'left:0') : '';
  return `<div class="pcard ${ef.cur === t.id ? 'kb' : ''} ${t.done ? 'done' : ''}" draggable="true" data-drag="${t.id}" data-tid="${t.id}">${esc(t.title)}<div class="m">${q ? `<span class="dot" style="--c:${q.color}"></span>` : '<span class="faint">Inbox</span>'}${marks}</div>${pop}</div>`;
}
const dueMark = (t, mobile) => {
  const q = primary(t), over = t.due < 0;
  return `<div class="pdue ${over ? 'over' : ''}" style="--c:${q ? q.color : 'var(--faint)'}" title="Deadline${has(t.plan) ? ', planned ' + dayName(t.plan) : ''}"><b>◆</b><span>${mobile ? 'Due: ' : ''}${esc(t.title)}</span></div>`;
};
function planDesk() {
  // A span runs from the planned day to the due day, so start and deadline read together.
  const spans = openTasks().filter(t => has(t.plan) && has(t.due) && t.due > Math.max(t.plan, 0))
    .map(t => ({ t, a: Math.max(0, t.plan), b: Math.min(6, t.due) }));
  return `<div class="plan">
    <div class="pgrid">${WEEK.map(d => `<div class="pday ${d === 0 ? 'today' : ''}"><b>${d === 0 ? 'Today' : wd(d)}</b> ${dnum(d)}</div>`).join('')}</div>
    ${spans.length ? `<div class="pgrid pspans">${spans.map(({ t, a, b }) => `<div class="pspan" style="grid-column:${a + 1} / ${b + 2};--c:${(primary(t) || { color: 'var(--faint)' }).color}" title="${esc(t.title)}: planned ${dayName(t.plan)}, due ${dayName(t.due)}"><i></i><span class="ell">${esc(t.title)}</span></div>`).join('')}</div>` : ''}
    <div class="pgrid pcols">${WEEK.map(d => `<div class="pcol ${d === 0 ? 'today' : ''}" data-drop="${d}">${plannedOn(d).map(planCard).join('')}${dueOn(d).filter(t => !plannedOn(d).includes(t)).map(t => dueMark(t)).join('')}</div>`).join('')}</div>
    <div class="ptray" data-drop="none"><div class="lbl2">Not planned</div><div class="items">${unplanned().map(planCard).join('')}</div></div>
  </div>`;
}
function planMobile() {
  return WEEK.map(d => {
    const rows = plannedOn(d).map(efRow).join(''), dues = dueOn(d).filter(t => !plannedOn(d).includes(t)).map(t => dueMark(t, true)).join('');
    return `<div class="grp">${dayName(d)} <span class="faint" style="font-weight:500">${wd(d)} ${dnum(d)}</span></div>${rows}${dues ? `<div style="display:grid;gap:6px;margin:6px 0">${dues}</div>` : ''}${rows || dues ? '' : '<div class="faint small" style="padding:8px 0">–</div>'}`;
  }).join('') + `<div class="grp">Not planned</div>${unplanned().map(efRow).join('')}`;
}

/* ---------- Quick-Add: "#quest" files it, "@fri" plans it, "!fri" sets the deadline ----------
   Focus unfolds the same field row as the task detail; the tokens fill it. Its defaults show what ↵ will do. */
const qa = { text: params.get('qa') || '', caret: null, idx: 0, quest: null, plan: null, due: null, view: undefined, focused: params.get('qaopen') === '1' };
const qaDefaults = view => ({ quest: view.startsWith('q:') ? view.slice(2) : null, plan: view === 'today' ? 0 : null, due: null });
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
  const t = { id: 't' + (++seq), title, quests, plan, due, day: plan === 0 ? 'today' : plan > 0 ? 'upcoming' : null, when: plan > 0 ? dayName(plan) : undefined };
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

/* ---------- Drag a task onto a day (desktop Plan) ---------- */
document.addEventListener('dragstart', e => {
  const card = e.target.closest && e.target.closest('[data-drag]');
  if (!card) return;
  e.dataTransfer.setData('text/plain', card.dataset.drag);
  e.dataTransfer.effectAllowed = 'move';
});
document.addEventListener('dragover', e => {
  const zone = e.target.closest && e.target.closest('[data-drop]');
  if (!zone) return;
  e.preventDefault();
  document.querySelectorAll('[data-drop].over').forEach(z => z !== zone && z.classList.remove('over'));
  zone.classList.add('over');
});
document.addEventListener('drop', e => {
  const zone = e.target.closest && e.target.closest('[data-drop]');
  if (!zone) return;
  e.preventDefault();
  const t = task(e.dataTransfer.getData('text/plain'));
  if (!t || t.builtin) return;
  changeDates(t, setPlanned(t, zone.dataset.drop === 'none' ? null : +zone.dataset.drop));
  toast(has(t.plan) ? `Planned for ${dayName(t.plan)}` : 'Unplanned');
  render();
});
document.addEventListener('dragend', () => document.querySelectorAll('[data-drop].over').forEach(z => z.classList.remove('over')));
