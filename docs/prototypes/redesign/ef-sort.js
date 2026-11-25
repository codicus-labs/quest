const orderField = group => group === 'today' || group.startsWith('plan:') ? 'dayOrder' : 'sortKey';
const orderedTasks = (tasks, group) => {
  const field = orderField(group);
  return [...tasks].sort((a, b) => (a[field] ?? 0) - (b[field] ?? 0));
};
function sortGroup(group, tasks) {
  return `<div data-sort-group="${group}">${orderedTasks(tasks, group).map(t => efRow(t, true)).join('')}</div>`;
}
const endOrder = (tasks, field) => Math.max(-1, ...tasks.map(t => t[field] ?? 0)) + 1;
function endDayOrder(t, except) {
  const onDay = isEfToday(t) ? isEfToday : other => has(t.plan) && t.plan === other.plan;
  return endOrder(liveT().filter(other => other.id !== except && !other.done && onDay(other)), 'dayOrder');
}
function endSortKey(questId, except) {
  const belongs = questId ? t => t.quests.includes(questId) : isEfInbox;
  return endOrder(liveT().filter(t => t.id !== except && !t.done && belongs(t)), 'sortKey');
}

const sortRows = group => [...group.querySelectorAll(':scope > .item > [data-drag]')];
const moveFocusToTask = t => { ef.cur = t.id; ef.revealSelection = true; };
function placeTask(t, group, before) {
  const key = group.dataset.sortGroup, field = orderField(key), rows = sortRows(group);
  const remaining = rows.filter(row => row.dataset.drag !== t.id);
  const at = before ? remaining.indexOf(before) : remaining.length;
  if (at === rows.findIndex(row => row.dataset.drag === t.id)) return;
  const previous = remaining[at - 1], next = remaining[at];
  const lower = previous ? (task(previous.dataset.drag)[field] ?? 0) : null;
  const upper = next ? (task(next.dataset.drag)[field] ?? 0) : null;
  const order = lower === null ? (upper ?? 1) - 1 : upper === null ? lower + 1 : lower + (upper - lower) / 2;
  if (key.startsWith('plan:') && !rows.some(row => row.dataset.drag === t.id)) {
    changeDates(t, { ...setPlanned(t, +key.slice(5)), dayOrder: order });
  } else {
    undoableTaskChange(t, null, () => { t[field] = order; });
  }
  moveFocusToTask(t);
}
function moveTaskInGroup(delta) {
  const row = ef.cur && document.querySelector(`[data-drag="${ef.cur}"]`);
  const group = row?.closest('[data-sort-group]');
  if (!group) return false;
  const rows = sortRows(group), from = rows.indexOf(row), to = from + delta;
  if (to >= 0 && to < rows.length) placeTask(task(ef.cur), group, rows[to + (delta > 0 ? 1 : 0)] || null);
  return true;
}

function fileTask(t, target) {
  if (target === 'today') {
    changeDates(t, { ...setPlanned(t, 0), dayOrder: endDayOrder({ ...t, plan: 0 }, t.id) });
  } else if (target === 'inbox') {
    changeDates(t, { ...setPlanned(t, null), quests: [], sortKey: endSortKey(null, t.id) });
  } else {
    const id = target.slice(2);
    undoableTaskChange(t, 'Quest change', () => {
      t.quests = [id];
      t.sortKey = endSortKey(id, t.id);
    });
  }
}

let dragging = null, destination = null;
function startTaskDrag(row) {
  dragging = { row, task: task(row.dataset.drag) };
  row.classList.add('dragging');
}
function dropAt(el, y) {
  const sidebar = el?.closest('.side [data-file]');
  if (sidebar) return { marker: sidebar, cls: 'file-over', sidebar };
  const group = el?.closest('[data-sort-group]') || el?.closest('[data-drop]')?.querySelector('[data-sort-group]');
  if (!group) return null;
  const rows = sortRows(group).filter(row => row !== dragging.row);
  const before = rows.find(row => {
    const bounds = row.getBoundingClientRect();
    return y < bounds.top + bounds.height / 2;
  });
  if (before) return { marker: before, cls: 'insert-before', group, before };
  if (rows.length) return { marker: rows.at(-1), cls: 'insert-after', group, before: null };
  return { marker: group, cls: 'insert-after', group, before: null };
}
function markDestination(next) {
  if (destination?.marker !== next?.marker || destination?.cls !== next?.cls) {
    destination?.marker.classList.remove(destination.cls);
    next?.marker.classList.add(next.cls);
  }
  destination = next;
}
function finishTaskDrag(drop) {
  if (!dragging) return;
  const t = dragging.task, target = destination;
  dragging.row.classList.remove('dragging');
  dragging = null;
  markDestination(null);
  if (!drop || !target) return;
  ef.pop = null;
  if (target.sidebar) fileTask(t, target.sidebar.dataset.file);
  else placeTask(t, target.group, target.before);
  render();
}

document.addEventListener('dragstart', e => {
  const row = e.target.closest('.line[draggable="true"]');
  if (!row) return;
  startTaskDrag(row);
  e.dataTransfer.setData('text/plain', dragging.task.title);
  e.dataTransfer.effectAllowed = 'move';
});
document.addEventListener('dragover', e => {
  if (!dragging) return;
  markDestination(dropAt(e.target, e.clientY));
  if (!destination) return;
  e.preventDefault();
  e.dataTransfer.dropEffect = 'move';
});
document.addEventListener('drop', e => {
  if (!dragging) return;
  markDestination(dropAt(e.target, e.clientY));
  if (destination) e.preventDefault();
  finishTaskDrag(true);
});
document.addEventListener('dragend', () => finishTaskDrag(false));

let hold = null, ignoreTouchClick = false;
function moveLiftedTask() {
  const { x, y, ghost, scroll } = hold;
  const bounds = scroll.getBoundingClientRect();
  const left = Math.max(bounds.left + 12, Math.min(x + 12, bounds.right - ghost.offsetWidth - 12));
  ghost.style.transform = `translate(${left}px, ${y - 42}px)`;
  const top = bounds.top + (document.querySelector('.pweek')?.offsetHeight || 0);
  const bottom = document.querySelector('.efplayer').getBoundingClientRect().top;
  if (y < top + 40) scroll.scrollTop -= 8;
  else if (y > bottom - 40) scroll.scrollTop += 8;
  markDestination(dropAt(document.elementFromPoint(x, y), y));
  hold.frame = requestAnimationFrame(moveLiftedTask);
}
function liftTask() {
  startTaskDrag(hold.row);
  hold.ghost = document.createElement('div');
  hold.ghost.className = 'task-lift';
  hold.ghost.textContent = dragging.task.title;
  document.body.append(hold.ghost);
  moveLiftedTask();
}
function endHold(drop) {
  if (!hold) return;
  clearTimeout(hold.timer);
  cancelAnimationFrame(hold.frame);
  if (hold.ghost) {
    hold.ghost.remove();
    ignoreTouchClick = true;
    finishTaskDrag(drop);
  }
  hold = null;
}
document.addEventListener('touchstart', e => {
  ignoreTouchClick = false;
  if (hold) endHold(false);
  if (ui.device !== 'mobile' || e.touches.length !== 1 || e.target.closest('button, input, textarea')) return;
  const row = e.target.closest('[data-drag]');
  if (!row) return;
  const { clientX: x, clientY: y } = e.touches[0];
  hold = { row, x, y, scroll: row.closest('.scroll'), timer: setTimeout(liftTask, 400) };
}, { passive: true });
document.addEventListener('touchmove', e => {
  if (!hold) return;
  const { clientX: x, clientY: y } = e.touches[0];
  if (!hold.ghost && Math.hypot(x - hold.x, y - hold.y) > 8) return endHold(false);
  if (hold.ghost) e.preventDefault();
  Object.assign(hold, { x, y });
}, { passive: false });
document.addEventListener('touchend', e => {
  if (hold?.ghost) {
    e.preventDefault();
    const { clientX: x, clientY: y } = e.changedTouches[0];
    markDestination(dropAt(document.elementFromPoint(x, y), y));
  }
  endHold(true);
}, { passive: false });
document.addEventListener('touchcancel', () => endHold(false));
document.addEventListener('contextmenu', e => { if (hold) e.preventDefault(); });
document.addEventListener('click', e => {
  if (!ignoreTouchClick) return;
  ignoreTouchClick = false;
  e.preventDefault();
  e.stopImmediatePropagation();
}, true);
