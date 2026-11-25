const PLAN_DAYS = 14;
let showUnplanned = false, pendingPlanJump = null;

const planDayLabel = day => day < 2 ? dayName(day) : dateOf(day).toLocaleDateString('en-GB', { weekday: 'long' });
const planDayDate = day => dateOf(day).toLocaleDateString('en-GB', { dateStyle: 'full' });
const planAddView = day => `plan:${day}`;
const notPlanned = t => !has(t.plan) && t.quests.length > 0;

function planByDay(tasks) {
  const days = new Map([[0, { planned: efListOf('today'), deadlines: [] }]]);
  const forDay = day => {
    if (!days.has(day)) days.set(day, { planned: [], deadlines: [] });
    return days.get(day);
  };
  for (const t of tasks) {
    if (t.plan > 0) forDay(t.plan).planned.push(t);
  }
  for (const t of tasks) {
    if (!has(t.due)) continue;
    const day = forDay(Math.max(0, t.due));
    if (!day.planned.includes(t)) day.deadlines.push(t);
  }
  return days;
}

function planDaySection(day, bucket) {
  const { planned = [], deadlines = [] } = bucket || {};
  const view = planAddView(day), adding = qa.view === view && qaOpen();
  const rows = taskListRows(`plan:${day}`, planned) + deadlines.map(t =>
    `<div class="pdue" title="${planDayDate(t.due)}">${ic('flag')}<span>Due: ${esc(t.title)}</span></div>`).join('');
  return `<section class="psection ${day === 0 ? 'today' : ''} ${planned.length || deadlines.length ? 'has-rows' : ''}" id="plan-day-${day}" data-drop="${day}">
    <header class="phead"><h2><time title="${planDayDate(day)}"><b class="tnum">${dnum(day)}</b>${planDayLabel(day)}</time></h2>
      <button data-a="planAdd" data-day="${day}" aria-label="Add to-do for ${planDayDate(day)}" aria-expanded="${adding}">${ic('plus')}</button></header>
    ${adding ? efQuickAddFor(view) : ''}${rows}</section>`;
}

function planWeekStrip(days) {
  const monday = -((dateOf(0).getDay() + 6) % 7);
  const weeks = [];
  for (let start = monday; start < PLAN_DAYS; start += 7) {
    const buttons = Array.from({ length: 7 }, (_, i) => {
      const day = start + i, bucket = days.get(day);
      const busy = !!(bucket?.planned.length || bucket?.deadlines.length);
      return `<button data-a="planJump" data-day="${day}" aria-label="${planDayDate(day)}${busy ? ', tasks scheduled' : ''}"${day === 0 ? ' aria-current="date"' : ''}${day < 0 || day >= PLAN_DAYS ? ' disabled' : ''}>
        <span>${wd(day)}</span><b class="tnum">${dnum(day)}</b><i class="${busy ? 'has-tasks' : ''}"></i></button>`;
    });
    weeks.push(`<div>${buttons.join('')}</div>`);
  }
  return `<nav class="pweek" data-scroll="plan-weeks" aria-label="Plan weeks">${weeks.join('')}</nav>`;
}

function planList() {
  const tasks = liveT().filter(looksOpen), days = planByDay(tasks);
  const daily = Array.from({ length: PLAN_DAYS }, (_, day) => planDaySection(day, days.get(day))).join('');
  const months = new Map();
  const monthLabel = day => dateOf(day).toLocaleDateString('en-GB', { month: 'long', ...(dateOf(day).getFullYear() !== dateOf(0).getFullYear() && { year: 'numeric' }) });
  const lastDaily = monthLabel(PLAN_DAYS - 1);
  if (monthLabel(PLAN_DAYS) === lastDaily) months.set(`Rest of ${lastDaily}`, '');
  for (const day of [...days.keys()].filter(day => day >= PLAN_DAYS).sort((a, b) => a - b)) {
    const month = monthLabel(day), label = month === lastDaily ? `Rest of ${month}` : month;
    months.set(label, (months.get(label) || '') + planDaySection(day, days.get(day)));
  }
  const later = [...months].map(([label, rows]) => `<section class="pmonth"><h2>${label}</h2>${rows}</section>`).join('');
  const unplanned = tasks.filter(notPlanned);
  const tray = `<section class="pnot"><button class="pnot-toggle" data-a="planUnplanned" aria-expanded="${showUnplanned}">${ic(showUnplanned ? 'chev' : 'right')}Not planned <span class="faint tnum">${unplanned.length}</span></button>
    ${showUnplanned ? unplanned.map(t => efRow(t, true)).join('') : ''}</section>`;
  return `<div class="plan">${ui.device === 'mobile' ? planWeekStrip(days) : ''}${daily}${later}${tray}</div>`;
}

function jumpToPlanDay(day) { ef.cur = null; st.sel = null; pendingPlanJump = day; }
function openDayAdd(day) {
  if (qa.view !== planAddView(day)) resetQa(planAddView(day));
  qa.focused = true;
  focusQa();
}
Object.assign(actions, {
  planAdd: d => openDayAdd(+d.day),
  planJump: d => jumpToPlanDay(+d.day),
  planUnplanned: () => {
    showUnplanned = !showUnplanned;
    if (!showUnplanned && ef.cur && notPlanned(task(ef.cur))) { ef.cur = null; st.sel = null; }
  },
});

