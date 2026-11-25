use quest_domain::EntityId;
use rune::{Cursor, El, Tx, icon, row, text};
use rune_kit::{Combobox, ComboboxOption, ComboboxState};

use crate::state::State;
use crate::theme::{Pal, ty};

const CREATE: &str = "create";

#[derive(Clone, Copy)]
pub(super) enum Form {
    LogWork,
    Entry,
}

impl Form {
    fn picker(self, s: &mut State) -> &mut ComboboxState {
        match self {
            Self::LogWork => &mut s.log_work.task_picker,
            Self::Entry => &mut s.entry_form.task_picker,
        }
    }

    fn choose(self, s: &mut State, id: &str) {
        if id == CREATE {
            let title = self.picker(s).search().text.trim().to_owned();
            if title.is_empty() {
                return;
            }
            match self {
                Self::LogWork => {
                    s.log_work.task = None;
                    s.log_work.title = rune::TextEdit::new(title);
                    s.log_work.error = None;
                }
                Self::Entry => {
                    s.entry_form.task = None;
                    s.entry_form.new_task = Some(title);
                }
            }
            return;
        }
        let task = id.parse::<EntityId>().ok();
        match (self, task) {
            (Self::Entry, task) => s.pick_entry_task(task),
            (Self::LogWork, Some(task)) => s.pick_log_work_task(task),
            (Self::LogWork, None) => {}
        }
    }
}

pub(super) fn combo(form: Form, s: &State, p: &Pal, popups: &mut Vec<El>) -> El {
    let (key, state, task, new_task, empty) = match form {
        Form::LogWork => (
            "log-work-task",
            &s.log_work.task_picker,
            s.log_work.task,
            Some(s.log_work.title.text.as_str()),
            "Choose task…",
        ),
        Form::Entry => (
            "entry-editor-task",
            &s.entry_form.task_picker,
            s.entry_form.task,
            s.entry_form.new_task.as_deref(),
            "No task",
        ),
    };
    let new_task = new_task.filter(|title| !title.is_empty() && task.is_none());
    let selected = task.map(|id| id.to_string()).or_else(|| new_task.map(|_| CREATE.to_owned()));
    let query = state.search().text.trim();
    let needle = query.to_lowercase();
    let hits = state.is_open().then(|| crate::log_work::suggestions(&s.snap, query));
    let has_results = hits.as_ref().is_some_and(|hits| !hits.is_empty());
    let mut options = Vec::new();
    if state.is_open() || task.is_some() {
        let mut tasks: Vec<_> = s
            .snap
            .tasks
            .iter()
            .filter(|t| {
                Some(t.id) == task
                    || (state.is_open() && needle.is_empty())
                    || hits.as_ref().is_some_and(|hits| hits.iter().any(|hit| hit.id == t.id))
            })
            .collect();
        tasks.sort_by_key(|t| (t.is_done(), std::cmp::Reverse(t.completed_at)));
        options.extend(tasks.into_iter().map(|task| {
            let status = if task.is_done() { "Done" } else { "Open" };
            let mut option = ComboboxOption::new(task.id.to_string(), &task.title);
            option.detail = status.into();
            option
        }));
    }
    if state.is_open() && !query.is_empty() && !has_results {
        let mut create = ComboboxOption::new(CREATE, format!("Create “{query}”"));
        create.detail = "New completed task".into();
        options.insert(0, create);
    } else if let Some(title) = new_task {
        options.push(ComboboxOption::new(CREATE, title));
    }
    let shown = new_task.or_else(|| task.and_then(|id| s.snap.task(id)).map(|task| task.title.as_str()));
    let label = shown.map_or_else(|| "Choose task".to_owned(), |title| format!("Change task: {title}"));
    let trigger = row()
        .key(format!("{key}-value"))
        .role("combobox")
        .label(label)
        .focusable(true)
        .w_full()
        .h(42.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.bg)
        .border(1.0, p.border_strong)
        .hover(p.hover)
        .cursor(Cursor::Pointer)
        .child(
            text(shown.unwrap_or(empty))
                .font(ty::BODY)
                .color(if shown.is_some() { p.text } else { p.muted })
                .single_line()
                .truncate()
                .grow(1.0)
                .min_w(0.0)
                .a11y_hidden(),
        )
        .child(icon(p.kit.glyph.chevron_down, 14.0).color(p.muted).a11y_hidden());
    Combobox::new(key, state, &options, move |s: &mut State| form.picker(s))
        .label("Tasks")
        .trigger(trigger)
        .current(selected.as_deref())
        .placeholder(empty)
        .search_placeholder("Search or create a task…")
        .width(320.0)
        .on_choose(move |s, id, _: &mut Tx| form.choose(s, id))
        .build(&p.kit, popups)
        .grow(1.0)
        .min_w(0.0)
}
