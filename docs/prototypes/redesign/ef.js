/* PROTOTYPE – throwaway. Variants E and F: shell, tracked column, focus, player, account drawer.
   Lists, dates, Plan and Quick-Add live in ef-lists.js. Loaded after index.html's main script and reuses its globals. */

Object.assign(P, {
  panel: '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M14.5 4.5v15"/>',
  focus: '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="3"/>',
  out: '<path d="M14 5h4a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2h-4"/><path d="M10 8l-4 4 4 4M6 12h10"/>',
  gear: '<circle cx="12" cy="12" r="3"/><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1"/>',
  right: '<path d="M10 7l5 5-5 5"/>',
  undo: '<path d="M9 14L4 9l5-5"/><path d="M4 9h10a6 6 0 0 1 0 12h-3"/>',
  redo: '<path d="M15 14l5-5-5-5"/><path d="M20 9H10a6 6 0 0 0 0 12h3"/>',
  diamond: '<path d="M12 4l8 8-8 8-8-8z"/>',
  dot: '<circle cx="12" cy="12" r="4"/>',
});

/* ---------- State ---------- */
const SCENES = (params.get('scene') || '').split(','); // several at once, e.g. ?scene=global,org
const ef = {
  right: params.get('right') !== '0', // desktop tracked column, kept across sidebar navigation
  focus: SCENES.includes('focus'), // F: focus view open
  leftAt: null, // F: when the focus view was left while the task timer kept running
  hintOff: false, // F: "keep on task" chosen for the current stretch outside focus
  hintSeen: false,
  drawer: params.get('drawer') === '1',
  cur: params.get('cur'), // keyboard selection (task id)
  pop: null, // open field picker: { target: task id | 'qa', kind: 'quest' | 'plan' | 'due', ... } (ef-fields.js)
  keys: params.get('keys') === '1', // shortcut overview
};
const variantKey = () => VARIANTS[ui.v][0];
const isEF = () => ['E', 'F'].includes(variantKey());
const strict = () => variantKey() === 'E';
const runningTask = () => (run && run.task !== ORG.id ? task(run.task) : null);
const focusOpen = () => !!runningTask() && (strict() || ef.focus);
const hintDue = () => !strict() && runningTask() && !ef.focus && ef.leftAt && !ef.hintOff && now() - ef.leftAt >= 120e3;
const orgMins = () => E.filter(e => e.task === ORG.id).reduce((s, e) => s + e.mins, 0) + (run && run.task === ORG.id ? runMins() : 0);

function openFocus() { Object.assign(ef, { focus: true, leftAt: null, hintOff: false, hintSeen: false }); }
function leaveFocus() { Object.assign(ef, { focus: false, leftAt: now(), hintOff: false, hintSeen: false }); }
function syncFocus() {
  // F: a task timer running outside the focus view always has a "left at" mark, also after switching variants.
  if (!strict() && runningTask() && !ef.focus && !ef.leftAt) ef.leftAt = now();
}

/* Demo scenes for screenshots: ?scene=org (after "Organize" / rebooking), hint (F, 2+ min outside focus), focus (F, view open), idle (no timer). */
(function applyScene() {
  if (SCENES.includes('org') && run) {
    const from = run.task, cut = run.start + 14 * 60e3;
    E.push({ task: from, start: run.start, mins: 14 });
    run = { task: ORG.id, start: cut, meta: { from, how: variantKey() === 'F' ? 'rebooked' : 'escape' } };
  }
  if (SCENES.includes('hint')) ef.leftAt = now() - 130e3;
  if (SCENES.includes('idle') && run) { E.push({ task: run.task, start: run.start, mins: 36 }); lastRun = run.task; run = null; }
})();

/* ---------- Tracked today + week ---------- */
function efBlocks() {
  const blocks = E.filter(e => isToday(e.start)).map(e => ({ ...e, end: e.start + e.mins * 60e3 }));
  if (!run) return blocks;
  if (runningTask() && !strict() && !ef.focus && ef.leftAt) {
    blocks.push({ task: run.task, start: run.start, end: ef.leftAt, run: true });
    blocks.push({ task: run.task, start: ef.leftAt, end: now(), run: true, outside: true });
  } else blocks.push({ task: run.task, start: run.start, end: now(), run: true, ...run.meta });
  return blocks;
}
function efBlock(b, y) {
  const t = task(b.task), q = primary(t), org = b.task === ORG.id;
  const top = y(b.start), height = Math.max(b.outside ? 6 : 3, y(b.end) - top);
  const from = b.from && task(b.from);
  const sub = org ? (b.how === 'rebooked' ? `rebooked from “${esc(from.title)}”` : b.how === 'escape' ? `left focus on “${esc(from.title)}”` : `${hm(b.start)}–${b.run ? 'now' : hm(b.end)}`)
    : b.outside ? 'outside focus · still on this task' : `${hm(b.start)}–${b.run ? 'now' : hm(b.end)}`;
  const how = b.how === 'rebooked' ? ' · rebooked' : b.how === 'escape' ? ' · left focus' : '';
  const range = `${hm(b.start)}–${b.run ? 'now' : hm(b.end)}`;
  const label = height < 12 || b.outside ? '' : `<b class="ell" style="display:block;font-weight:600">${org ? 'Organizing' : esc(t.title)}</b>${height > 32 ? `<div class="s ell tnum">${range}</div>` : ''}`;
  const cls = ['blk', height < 20 && 'thin', org && 'org', b.how === 'rebooked' && 'rebooked', b.outside && 'outside'].filter(Boolean).join(' ');
  return `<div class="${cls}" style="--c:${org ? 'var(--faint)' : q ? q.color : 'var(--faint)'};top:${top}px;height:${height}px" title="${esc(org ? 'Organizing' + how + ': ' : t.title + ': ')}${sub}">${label}</div>`;
}
function efTimeline(hFrom, hTo, px) {
  const y = ms => ((ms - at(0, hFrom)) / 3600e3) * px;
  let h = '';
  for (let k = hFrom; k <= hTo; k++) h += `<span class="hr" style="top:${(k - hFrom) * px}px">${pad(k)}:00</span><span class="hl" style="top:${(k - hFrom) * px}px"></span>`;
  h += efBlocks().map(b => efBlock(b, y)).join('') + `<div class="now" style="top:${y(now())}px"></div>`;
  return `<div class="eftl" style="height:${(hTo - hFrom) * px}px">${h}</div>`;
}
function efWeek() {
  const qm = questWeekMins(), org = orgMins(), other = Math.max(0, weekMins() - qm - org);
  return `<div class="efwk"><div class="row" style="align-items:baseline"><span class="h2 grow">This week</span><span class="tnum muted small">${fmtH(qm)} / ${WEEK_GOAL_H}h</span></div>
    ${bar(qm / 60 / WEEK_GOAL_H, 'var(--acc)')}
    ${Q.map(q => { const m = questMins(q.id); return `<div class="efwq"><span class="row ell" style="gap:7px"><span class="dot" style="--c:${q.color}"></span>${esc(q.title)}</span><span class="tnum muted">${fmtH(m)} / ${q.goal}h</span>${bar(m / 60 / q.goal, q.color)}</div>`; }).join('')}
    <div class="efwq org"><span class="row" style="gap:7px"><span class="dot" style="--c:var(--faint)"></span>Organizing</span><span class="tnum">${fmtH(org)}</span></div>
    ${other >= 1 ? `<div class="efwq" style="color:var(--muted)"><span class="row" style="gap:7px"><span class="dot" style="--c:var(--line2)"></span>No quest</span><span class="tnum">${fmtH(other)}</span></div>` : ''}
    <div class="efwq post-wk" title="Plan dates and deadlines moved later this week"><span class="row" style="gap:7px"><span class="post">${ic('redo')}</span>Postponed</span><span class="tnum">${postponedThisWeek()}</span></div></div>`;
}
function efTracked(mobile) {
  return `<div><div class="h2">Tracked today</div><div class="total tnum" data-today></div>
    ${mobile ? efTimeline(8, 16, 44) : efTimeline(8, 18, 38)}</div>${mobile ? '<div style="height:26px"></div>' : ''}${efWeek()}`;
}

/* ---------- Player (desktop: card in the sidebar, mobile: above the tab bar) ---------- */
function efHint(mobile) {
  if (!hintDue()) return '';
  return `<div class="efhint ${mobile ? 'efmhint' : ''}">${ic('clock')}<div class="grow"><div>Organizing since ${hm(ef.leftAt)}?</div>
    <div class="row" style="margin-top:8px;gap:4px"><button class="efbtn acc" data-a="rebook" title="Book the time since ${hm(ef.leftAt)} on Organizing">Book</button><button class="efbtn" data-a="hintOff" title="Keep the time on the task">Keep</button></div></div></div>`;
}
const backTo = from => from ? `<button class="backto" data-a="play" data-id="${from.id}">${ic('undo')}<span class="ell">Back to “${esc(from.title)}”</span></button>` : '';
function efTimerCard() {
  if (!run) return aTimer();
  if (run.task === ORG.id) {
    return `<div class="timer"><div class="row lbl3"><span class="dot pulse" style="--c:var(--faint)"></span>Organizing · no quest</div>
      <div class="row" style="margin-top:4px"><span class="t tnum" data-elapsed></span><span class="grow"></span><button class="play on gray" data-a="play" data-id="${ORG.id}">${ic('stop')}Stop</button></div>
      ${backTo(run.meta && task(run.meta.from))}</div>`;
  }
  const t = task(run.task), q = primary(t);
  return `<div class="timer"><div class="row lbl3">${q ? `<span class="dot pulse" style="--c:${q.color}"></span>${esc(q.title)}` : 'Tracking'}</div>
    <div class="ell" style="font-weight:600;margin:2px 0 4px">${esc(t.title)}</div>
    <div class="row"><span class="t tnum" data-elapsed></span><span class="grow"></span><button class="play" data-a="focusOpen" title="Open focus">${ic('focus')}</button><button class="play on" data-a="play" data-id="${t.id}">${ic('stop')}Stop</button></div></div>`;
}
function efPlayer() {
  if (!run) {
    const t = task(lastRun);
    return `<div class="pmain"><div class="muted small">Resume</div><div class="ell" style="font-weight:600">${esc(t.title)}</div></div><button class="pstop" data-a="play" data-id="${t.id}">${ic('play')}</button>`;
  }
  if (run.task === ORG.id) {
    return `<div class="pmain"><div class="row small muted" style="gap:6px"><span class="dot pulse" style="--c:var(--faint)"></span>Organizing <span class="pclock tnum" style="font-size:12px" data-elapsed></span></div>${backTo(run.meta && task(run.meta.from))}</div>
      <button class="pstop gray" data-a="play" data-id="${ORG.id}">${ic('stop')}</button>`;
  }
  const t = task(run.task), q = primary(t);
  return `<button class="pmain" data-a="focusOpen"><div class="row small muted" style="gap:6px"><span class="dot pulse" style="--c:${q ? q.color : 'var(--acc)'}"></span><span class="ell">${q ? esc(q.title) : 'No quest'}</span></div><div class="ell" style="font-weight:600">${esc(t.title)}</div></button>
    <span class="pclock tnum" data-elapsed></span><button class="pstop" data-a="play" data-id="${t.id}">${ic('stop')}</button>`;
}

/* ---------- Focus ---------- */
function efFocus(mobile) {
  const t = runningTask(), q = primary(t), soft = !strict();
  const today = E.filter(e => e.task === t.id && isToday(e.start)).reduce((s, e) => s + e.mins, 0) + runMins();
  const organize = `<button class="flink" data-a="organize" title="Track the next minutes as Organizing">${ic('list')} Organize</button>`;
  const leave = `<div class="row fesc">${soft ? `<button class="flink" data-a="focusLeave" title="The timer keeps running">${ic('back')} Back to lists</button>` : ''}${organize}</div>`;
  return `<div class="effocus ${mobile ? 'm' : ''}">
    <div class="fhead">${logo()}<span>Focus</span><span class="grow"></span>${soft ? `<button class="iconbtn" style="margin:0" data-a="focusLeave" title="Leave focus (Esc)">${ic('x')}</button>` : ''}</div>
    <div class="fbody">
      <div class="fq">${q ? `<span class="dot" style="--c:${q.color}"></span>${esc(q.title)}` : 'No quest'}</div>
      <div class="ftitle">${esc(t.title)}</div>
      <div class="fclock tnum" data-elapsed></div>
      <div class="muted small tnum">${fmtDur(today)} today${q ? ` · ${fmtH(questMins(q.id))} / ${q.goal}h this week` : ''}</div>
      ${noteBlock(t, 'focus')}${checklist(t, 'focus')}
      <div class="row factions"><button class="efbig" data-a="focusStop">${ic('stop')}Stop</button><button class="efbig acc" data-a="focusDone">${ic('check')}Done</button></div>
      ${leave}
    </div></div>`;
}

/* ---------- Account drawer (mobile) ---------- */
function efDrawer() {
  return `<div class="efbg" data-a="drawer"></div><div class="efsheet"><div class="grab"></div>
    <div class="dsec">Workspace</div><div class="dcard">
      ${WORKSPACES.map(w => `<button class="drow ${w === workspace ? 'on' : ''}" data-a="ws" data-w="${w}"><span class="wsq">${w[0]}</span><span class="grow">${w}</span>${w === workspace ? `<span style="color:var(--acc-text)">${ic('check')}</span>` : ''}</button>`).join('')}
      <button class="drow" data-a="wsNew">${ic('plus')}<span class="muted">New workspace</span></button></div>
    <div class="dsec">Account</div><div class="dcard">
      <div class="drow"><span class="av">D</span><div class="grow"><div>Demo User</div><div class="faint small">sync.example.org · synced just now</div></div></div>
      <button class="drow" data-a="signout">${ic('out')}<span>Sign out</span></button></div>
    <div class="dsec">Settings</div><div class="dcard">
      <button class="drow" data-a="efTheme">${ic('sun')}<span class="grow">Appearance</span><span class="faint small">${ui.theme === 'dark' ? 'Dark' : 'Light'}</span></button>
      <button class="drow" data-a="noop">${ic('flag')}<span class="grow">Weekly goals</span>${ic('right', 'faint')}</button>
      <button class="drow" data-a="noop">${ic('gear')}<span class="grow">All settings</span>${ic('right', 'faint')}</button></div>
  </div>`;
}

/* ---------- Shells ---------- */
function efDesk() {
  syncFocus();
  const wide = st.view === 'plan';
  return `<aside class="side">
    <div style="position:relative;margin-bottom:12px">${wsBtn()}${wsMenu('top:40px;left:0')}</div>
    ${efNav()}<div class="sec">Quests</div>${aQuestNav()}
    <div style="height:14px;margin-top:auto"></div>${efHint(false)}${efTimerCard()}
  </aside>
  <main class="main" data-scroll="ef-main"><div class="eftop"><button class="iconbtn ${ef.right ? 'on' : ''}" data-a="efRight" title="${ef.right ? 'Hide' : 'Show'} tracked time">${ic('panel')}</button></div>
    <div class="col ${wide ? 'wide' : ''}">${efHead()}${efQuickAddFor(st.view)}${efList()}</div></main>
  ${ef.right ? `<aside class="efright" data-scroll="ef-right">${efTracked(false)}</aside>` : ''}
  ${focusOpen() ? efFocus(false) : ''}${ef.keys ? efKeys() : ''}`;
}
const EF_TABS = [['today', 'Today', 'sun'], ['inbox', 'Inbox', 'inbox'], ['quests', 'Quests', 'flag'], ['tracked', 'Tracked', 'clock']];
function efMobileBody() {
  if (st.tab === 'tracked') return `<h1 style="margin-bottom:10px">Tracked</h1>${efTracked(true)}`;
  if (st.tab === 'quests' && !st.view.startsWith('q:')) return `<h1 style="margin-bottom:14px">Quests</h1><div class="mhome">${aQuestNav()}</div>`;
  const back = st.view.startsWith('q:') ? `<button class="row" style="color:var(--acc-text);margin:4px 0 12px -4px;font-size:15px" data-a="eftab" data-t="quests">${ic('back')} Quests</button>` : '';
  const seg = st.tab === 'today' ? `<div class="efseg">${[['today', 'Today'], ['plan', 'Plan']].map(([v, l]) => `<button class="${st.view === v ? 'on' : ''}" data-a="nav" data-v="${v}">${l}</button>`).join('')}</div>` : '';
  return `${seg}${back}${efHead()}${efQuickAddFor(st.view)}${efList()}`;
}
function efMobile() {
  syncFocus();
  return `<div class="scroll" data-scroll="ef-m-${st.tab}-${st.view}"><div class="col">${efMobileBody()}</div></div>
    ${efHint(true)}<div class="efplayer">${efPlayer()}</div>
    <nav class="eftabs">${EF_TABS.map(([k, l, i]) => `<button class="${st.tab === k && !ef.drawer ? 'on' : ''}" data-a="eftab" data-t="${k}">${ic(i)}${l}</button>`).join('')}
      <button class="${ef.drawer ? 'on' : ''}" data-a="drawer" title="Account and workspaces"><span class="av">D</span><span class="ell">${workspace}</span></button></nav>
    ${ef.drawer ? efDrawer() : ''}${ef.pop ? efPopSheet() : ''}${focusOpen() ? efFocus(true) : ''}<div class="homebar"></div>`;
}
VIEWS.E = [efDesk, efMobile, 'vA vEF vE'];
VIEWS.F = [efDesk, efMobile, 'vA vEF vF'];

/* ---------- Actions ---------- */
const basePlay = actions.play, baseWs = actions.ws, baseToggle = actions.toggle;
Object.assign(actions, {
  play: d => { basePlay(d); if (isEF() && runningTask() && run.task === d.id) openFocus(); },
  toggle: d => { baseToggle(d); const t = task(d.id); t.doneToday = t.done; },
  ws: d => { baseWs(d); ef.drawer = false; },
  focusOpen: () => openFocus(),
  focusLeave: () => leaveFocus(),
  focusStop: () => { stopTimer(); ef.focus = false; },
  focusDone: () => { const t = runningTask(); stopTimer(); t.done = true; t.doneToday = true; ef.focus = false; toast(`Done: “${t.title}”`); },
  organize: () => {
    const from = run.task;
    stopTimer();
    run = { task: ORG.id, start: now(), meta: { from, how: 'escape' } };
    ef.focus = false;
  },
  rebook: () => {
    const from = run.task, cut = ef.leftAt;
    E.push({ task: from, start: run.start, mins: Math.max(1, Math.round((cut - run.start) / 60e3)) });
    run = { task: ORG.id, start: cut, meta: { from, how: 'rebooked' } };
    ef.leftAt = null;
  },
  hintOff: () => { ef.hintOff = true; },
  efRight: () => { ef.right = !ef.right; },
  eftab: d => { st.tab = d.t; st.sel = null; ef.drawer = false; if (d.t === 'today' || d.t === 'inbox') st.view = d.t; if (d.t === 'quests') st.view = 'quests'; },
  drawer: () => { ef.drawer = !ef.drawer; },
  wsNew: () => toast('New workspace: name it, then pick a server (not part of the prototype)'),
  signout: () => toast('Sign out (not part of the prototype)'),
  efTheme: () => { ui.theme = ui.theme === 'dark' ? 'light' : 'dark'; syncUrl(); },
});

document.addEventListener('keydown', e => {
  if (!isEF() || e.key !== 'Escape') return;
  if (!strict() && focusOpen()) { leaveFocus(); render(); }
  else if (ef.drawer) { ef.drawer = false; render(); }
});
document.addEventListener('input', e => { if (e.target.dataset.notes) task(e.target.dataset.notes).notes = e.target.value; });
document.getElementById('pill').addEventListener('click', e => {
  const b = e.target.closest('[data-p="skip"]'); if (!b) return;
  if (!ef.leftAt) return toast('Leave the focus view first (the timer must run outside focus)');
  ef.leftAt -= 120e3; run.start = Math.min(run.start, ef.leftAt); ef.hintSeen = false; render();
});
setInterval(() => { if (isEF() && hintDue() && !ef.hintSeen) { ef.hintSeen = true; render(); } }, 1000);
