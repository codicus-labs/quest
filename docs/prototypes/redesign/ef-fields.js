/* PROTOTYPE – throwaway. Variants E and F: the task's field row [Quest ▾] [Plan] [Deadline] and its pickers
   (quest combobox, month calendars). Desktop: popover under the field; mobile: bottom sheet.
   A picker targets a task or the Quick-Add ('qa'); dates go through setPlanned / setDue (ef-lists.js). */

const DAY_MS = 864e5;
const dateOf = off => new Date(noon(off));
const offOf = d => Math.round((new Date(d.getFullYear(), d.getMonth(), d.getDate(), 12) - noon(0)) / DAY_MS);
const nextMonday = () => ((1 - new Date(base).getDay() + 7) % 7) || 7;
const fieldDay = off => Math.abs(off) < 7 ? `${wd(off)} ${dnum(off)}` : dateOf(off).toLocaleDateString('en-GB', { day: 'numeric', month: 'short' });
const QUICK = [['Today', 0, 't'], ['Tomorrow', 1, 'm'], ['Next Monday', null, 'n'], ['None', '', '⌫']];
const quickOff = o => o === null ? nextMonday() : o;

/* Most recently used first (newest time entry of a quest's tasks), never used ones in page order. */
function recentQuests() {
  const last = {};
  for (const e of E) { const t = task(e.task); if (t && t.quests[0]) last[t.quests[0]] = Math.max(last[t.quests[0]] || 0, e.start + e.mins * 60e3); }
  if (runningTask() && primary(runningTask())) last[primary(runningTask()).id] = now();
  return [...Q].sort((a, b) => (last[b.id] || 0) - (last[a.id] || 0));
}
function questItems(query) {
  const q = query.trim().toLowerCase();
  const quests = recentQuests().filter(x => x.title.toLowerCase().includes(q)).map(x => x.id);
  return 'no quest'.includes(q) ? [...quests, ''] : quests;
}

/* ---------- Field row: task detail and Quick-Add share it ----------
   target is a task id or 'qa'; the Quick-Add has no postponements yet. */
const fieldSrc = target => target === 'qa' ? { plan: qa.plan, due: qa.due, quest: qa.quest } : (t => ({ plan: t.plan, due: t.due, quest: t.quests[0] }))(task(target));
function efFields(target) {
  const v = fieldSrc(target), q = v.quest && quest(v.quest), mobile = ui.device === 'mobile';
  const isOn = kind => ef.pop && ef.pop.target === target && ef.pop.kind === kind;
  const pop = kind => isOn(kind) && !mobile ? efPopover('left:0') : '';
  const post = kind => kind === 'quest' || target === 'qa' ? '' : postMark(target, kind);
  const field = (kind, inner, cls = '') => `<span class="fldw"><button class="fld ${cls} ${isOn(kind) ? 'on' : ''}" id="fld-${kind}-${target}" data-a="pop" data-id="${target}" data-k="${kind}">${inner}</button>${post(kind)}${pop(kind)}</span>`;
  return `<div class="efields">
    ${field('quest', q ? `<span class="dot" style="--c:${q.color}"></span><span class="ell">${esc(q.title)}</span>${ic('chev', 'faint')}` : `<span class="dot" style="--c:var(--line2)"></span><span class="ph">Quest</span>${ic('chev', 'faint')}`)}
    ${field('plan', `${ic('cal')}${has(v.plan) ? fieldDay(v.plan) : '<span class="ph">Plan</span>'}`, has(v.plan) ? '' : 'empty')}
    ${field('due', `${ic('flag')}${has(v.due) ? fieldDay(v.due) : '<span class="ph">Deadline</span>'}`, has(v.due) ? (v.due < 0 ? 'over' : '') : 'empty')}
  </div>`;
}

/* ---------- Picker bodies ---------- */
const popValues = () => fieldSrc(ef.pop.target);
function questBody() {
  const p = ef.pop, cur = popValues().quest, items = questItems(p.q);
  return `<input class="popq" id="efpop-q" value="${esc(p.q)}" placeholder="Quest" autocomplete="off" role="combobox" aria-expanded="true">
    <div class="popl" role="listbox">${items.map((id, i) => {
      const q = quest(id);
      return `<button class="${i === p.idx ? 'on' : ''}" role="option" data-a="popSet" data-q="${id}"><span class="dot" style="--c:${q ? q.color : 'var(--line2)'}"></span><span class="grow ell">${q ? esc(q.title) : 'No quest'}</span>${(cur || '') === id ? ic('check', 'acc') : ''}</button>`;
    }).join('') || '<div class="faint small" style="padding:8px 9px">–</div>'}</div>`;
}
function calBody() {
  const p = ef.pop, v = popValues(), sel = v[p.kind], other = p.kind === 'plan' ? v.due : v.plan;
  const first = new Date(base.getFullYear(), base.getMonth() + p.month, 1), lead = (first.getDay() + 6) % 7, start = offOf(first) - lead;
  const days = new Date(first.getFullYear(), first.getMonth() + 1, 0).getDate(), cells = Math.ceil((lead + days) / 7) * 7;
  const otherCls = p.kind === 'plan' ? 'mdue' : 'mplan';
  let grid = ['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su'].map(d => `<span class="cwd">${d}</span>`).join('');
  for (let i = 0; i < cells; i++) {
    const off = start + i, inMonth = dateOf(off).getMonth() === first.getMonth();
    const cls = [!inMonth && 'out', off === 0 && 'today', off === sel && 'sel', off === other && otherCls, off === p.cursor && 'cur'].filter(Boolean).join(' ');
    const tip = off === other ? ` title="${p.kind === 'plan' ? 'Deadline' : 'Planned'}"` : '';
    grid += `<button class="cday ${cls}" data-a="popSet" data-o="${off}" ${off < 0 ? 'disabled' : ''}${tip}>${dateOf(off).getDate()}</button>`;
  }
  return `<div class="qacts">${QUICK.map(([l, o, k]) => `<button class="${o === '' ? (has(sel) ? '' : 'on') : quickOff(o) === sel ? 'on' : ''}" data-a="popSet" data-o="${o === '' ? '' : quickOff(o)}">${l}<kbd>${k.toUpperCase()}</kbd></button>`).join('')}</div>
    <div class="cmhead"><button data-a="popMonth" data-m="-1" aria-label="Previous month" ${p.month <= 0 ? 'disabled' : ''}>${ic('back')}</button><b>${first.toLocaleDateString('en-GB', { month: 'long', year: 'numeric' })}</b><button data-a="popMonth" data-m="1" aria-label="Next month">${ic('right')}</button></div>
    <div class="cgrid ${p.kind}">${grid}</div>`;
}
const popBody = () => ef.pop.kind === 'quest' ? questBody() : calBody();
const efPopover = pos => `<div class="efpop ${ef.pop.kind}" id="efpop" tabindex="-1" style="${pos}">${popBody()}</div>`;
function efPopSheet() {
  const target = ef.pop.target, title = target === 'qa' ? qa.text.trim() : task(target).title;
  const head = `<div class="pshead ell ${title ? '' : 'faint'}">${title ? esc(title) : 'New to-do'}</div>${efFields(target)}`;
  return `<div class="efbg" data-a="popClose"></div><div class="efsheet efpopsheet" id="efpop" tabindex="-1"><div class="grab"></div>${head}<div class="psbody">${popBody()}</div></div>`;
}

/* ---------- Open, pick, close ---------- */
const monthOf = off => { const d = dateOf(off); return (d.getFullYear() - base.getFullYear()) * 12 + d.getMonth() - base.getMonth(); };
function openPop(target, kind, ret) {
  const src = target === 'qa' ? qa : task(target);
  const cursor = Math.max(0, has(src[kind]) ? src[kind] : kind === 'due' && has(src.plan) ? src.plan : 0);
  ef.pop = { target, kind, ret, q: '', idx: 0, cursor, month: monthOf(cursor), justOpened: true };
  if (target !== 'qa' && st.view !== 'plan') st.sel = target;
}
function closePop() {
  const ret = ef.pop.ret;
  ef.pop = null;
  if (ret === 'qa') focusQa(); else if (ret) ef.focusId = ret;
}
function applyPick(value) {
  const p = ef.pop;
  if (p.kind === 'quest' && p.target === 'qa') qa.quest = value || null;
  else if (p.kind === 'quest') task(p.target).quests = value ? [value] : [];
  else {
    const src = p.target === 'qa' ? qa : task(p.target);
    const next = p.kind === 'plan' ? setPlanned(src, value) : setDue(src, value);
    if (p.target === 'qa') Object.assign(qa, next); else changeDates(src, next);
  }
  closePop();
}
function moveCursor(days) {
  const p = ef.pop;
  p.cursor = Math.max(0, p.cursor + days);
  p.month = Math.max(0, monthOf(p.cursor));
}
function moveMonth(delta) {
  const p = ef.pop, d = dateOf(p.cursor), target = new Date(d.getFullYear(), d.getMonth() + delta, 1);
  const last = new Date(target.getFullYear(), target.getMonth() + 1, 0).getDate();
  moveCursor(offOf(new Date(target.getFullYear(), target.getMonth(), Math.min(d.getDate(), last))) - p.cursor);
  if (p.cursor === 0 && delta < 0) p.month = Math.max(0, p.month + delta);
}

Object.assign(actions, {
  pop: d => { if (ef.pop && ef.pop.target === d.id && ef.pop.kind === d.k) closePop(); else openPop(d.id, d.k, d.id === 'qa' ? 'qa' : `fld-${d.k}-${d.id}`); },
  popClose: () => closePop(),
  popSet: d => applyPick(d.q !== undefined ? d.q : d.o === '' ? null : +d.o),
  popMonth: d => { ef.pop.month = Math.max(0, ef.pop.month + +d.m); },
});

/* Clicks outside an open picker close it (capture: before the action of whatever was clicked). */
document.addEventListener('click', e => {
  if (!ef.pop || e.target.closest('.efpop, .efpopsheet, [data-a="pop"], [data-a="popClose"]')) return;
  ef.pop = null;
  if (!e.target.closest('[data-a]')) render();
}, true);

/* Keyboard inside a picker; runs before every other key handler. */
function popKey(e) {
  const p = ef.pop, k = e.key;
  if (k === 'Escape') { closePop(); return true; }
  if (p.kind === 'quest') {
    const items = questItems(p.q);
    if (k === 'ArrowDown' || k === 'ArrowUp') { p.idx = (p.idx + (k === 'ArrowDown' ? 1 : -1) + items.length) % Math.max(1, items.length); return true; }
    if (k === 'Enter' && items.length) { applyPick(items[p.idx]); return true; }
    return false;
  }
  const step = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 }[k];
  if (step) { moveCursor(step); return true; }
  if (k === 'PageUp' || k === 'PageDown') { moveMonth(k === 'PageUp' ? -1 : 1); return true; }
  if (k === 'Enter') { applyPick(p.cursor); return true; }
  if (e.metaKey || e.ctrlKey || e.altKey) return false;
  const quick = { t: 0, m: 1, n: nextMonday(), Backspace: null, Delete: null }[k.length === 1 ? k.toLowerCase() : k];
  if (quick !== undefined) { applyPick(quick); return true; }
  return false;
}
document.addEventListener('input', e => {
  if (e.target.id !== 'efpop-q') return;
  ef.pop.q = e.target.value; ef.pop.idx = 0; render();
});

/* After render: move focus into a freshly opened picker, or back to where it was opened from. */
const renderBeforeFields = render;
render = function () {
  renderBeforeFields();
  if (ef.pop && ef.pop.justOpened) {
    ef.pop.justOpened = false;
    const el = document.getElementById('efpop-q') || document.getElementById('efpop');
    if (el) el.focus();
  } else if (ef.focusId) {
    const el = document.getElementById(ef.focusId);
    ef.focusId = null;
    if (el) el.focus();
  }
  const q = document.getElementById('efpop-q');
  if (q && document.activeElement === q) q.setSelectionRange(q.value.length, q.value.length);
  const kb = document.querySelector('.vEF .kb');
  if (kb) kb.scrollIntoView({ block: 'nearest' });
};

/* Screenshot scenes: ?pop=t1:quest | t1:plan | qa:due opens a picker on load. */
if (params.get('pop')) { const [target, kind] = params.get('pop').split(':'); openPop(target, kind, null); }
