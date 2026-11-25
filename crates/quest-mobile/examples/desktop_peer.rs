//! A headless desktop replica for checking sync with the mobile app. Uses `quest-store`
//! directly (no Rune window), prints today's tasks with a timestamp whenever the store reports
//! a change, and takes commands on stdin:
//!
//!   add <title>                         create a task for today
//!   toggle <title>                      complete or reopen the task
//!   quest <title>                       create a quest with one task for today and 1.5h logged on it
//!   rename <old> => <new>
//!   describe <title> => <text>          set the description (\n for a line break)
//!   delete <title>                      soft-delete the task
//!   restore                             bring back the task deleted last
//!   start <title> | stop                the timer
//!   move <title> => <YYYY-MM-DD|none>     plan the task for a day, or unplan it
//!   due <title> => <YYYY-MM-DD|none>      set or clear the task's deadline
//!   repeat <title> => <day|week|month|year> [every]
//!   norepeat <title>
//!   assign <title> => <quest title|none>
//!   log <title> => HH:MM-HH:MM          log a session today on the task
//!   retime <title> => HH:MM-HH:MM       set the times of the task's latest session
//!   unlog <title>                       delete the task's latest session
//!   newquest <title>                    create an empty quest
//!   qcomplete <quest> | qreopen <quest> complete or reopen a quest
//!   qarchive <quest>                    archive it the way older versions did (to try Unarchive)
//!   qrename <old> => <new>
//!   qcolor <quest> => <red|orange|…>
//!   qdelete <quest>                     soft-delete the quest
//!   wsnew <name>                        create a workspace (and show it)
//!   wsswitch <name>                     show another live workspace
//!   wsrename <old> => <new> | wsdelete <name> | wsrestore <name>
//!   show                                print the state now
//!   quit
//!
//! A title names today's task with that title, else any task with it.
//!
//! cargo run -p quest-mobile --example desktop_peer -- <data-dir> <server-url> <user>
//!
//! Signs in once as <user> (e.g. anna) through the server and its identity provider without a browser
//! (`quest_oidc_fake::Browser`), so the server must use quest-oidc-fake as its OIDC provider. Tokens go
//! to <data-dir>/tokens.json (dev only); later runs reuse them.

use std::io::BufRead;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Instant;

use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use quest_domain::{
    Command, EntityId, EntityKind, EntryKind, Frequency, Position, Quest, QuestColor, QuestStatus, RepeatPin,
    RepeatRule, Snapshot, Task, TimeEntry, Weekdays, WorkTask,
};
use quest_oidc_fake::Browser;
use quest_store::{
    AppClient, FileVault, LoginRequest, Profiles, ProfilesConfig, SignInOutcome, Store, SyncStatus, Url,
};

/// Nothing listens there: the browser stand-in stops at the redirect.
const REDIRECT_PORT: u16 = 49153;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, server, user] = args.as_slice() else {
        eprintln!("usage: desktop_peer <data-dir> <server-url> <user>");
        std::process::exit(2);
    };
    let server: Url = server.parse().unwrap_or_else(|e| fail(&format!("invalid server URL: {e}")));
    // The stand-in browser's HTTP client needs a TLS provider, even for plain http.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let (events, inbox) = mpsc::channel::<Event>();
    let changed = events.clone();
    let dir = PathBuf::from(dir);
    let config = ProfilesConfig { vault: Arc::new(FileVault::new(dir.join("tokens.json"))), data_dir: dir };
    let (profiles, guest) = Profiles::open(config, move || {
        let _ = changed.send(Event::Changed);
    })
    .unwrap_or_else(|e| fail(&e.to_string()));
    let store = if guest.account().is_some() { guest } else { sign_in(&profiles, guest, &server, user) };
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            let _ = events.send(Event::Line(line));
        }
        let _ = events.send(Event::Line("quit".into()));
    });

    let started = Instant::now();
    let mut last = String::new();
    let mut last_deleted = None;
    for event in inbox {
        let line = match event {
            Event::Changed => {
                let now = describe(&store.status(), &store.snapshot());
                if now != last {
                    println!(
                        "[{} +{:.1}s] {now}",
                        Local::now().format("%H:%M:%S%.3f"),
                        started.elapsed().as_secs_f32()
                    );
                    last = now;
                }
                continue;
            }
            Event::Line(line) => line,
        };
        let (verb, rest) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
        let commands = match verb {
            "quit" => break,
            "show" => {
                println!("{}", describe(&store.status(), &store.snapshot()));
                continue;
            }
            "wsswitch" => {
                match workspace_named(&store.snapshot(), rest.trim(), false).map(|id| store.switch_workspace(id)) {
                    Ok(Ok(())) => println!("[{}] wsswitch {rest:?}", Local::now().format("%H:%M:%S%.3f")),
                    Ok(Err(e)) => println!("rejected: {e}"),
                    Err(message) => println!("{message}"),
                }
                continue;
            }
            "wsnew" => {
                let id = EntityId::new();
                let created = store.dispatch(Command::CreateWorkspace { id, name: rest.trim().to_owned() });
                match created.and_then(|()| store.switch_workspace(id)) {
                    Ok(()) => println!("[{}] wsnew {rest:?}", Local::now().format("%H:%M:%S%.3f")),
                    Err(e) => println!("rejected: {e}"),
                }
                continue;
            }
            "restore" => match last_deleted.take() {
                Some(id) => vec![Command::Restore { kind: EntityKind::Task, id }],
                None => {
                    println!("nothing deleted yet");
                    continue;
                }
            },
            _ => match commands_for(&store.snapshot(), verb, rest) {
                Ok(commands) => commands,
                Err(message) => {
                    println!("{message}");
                    continue;
                }
            },
        };
        if let Some(Command::DeleteTask { id, .. }) = commands.first() {
            last_deleted = Some(*id);
        }
        println!("[{}] {verb} {rest:?}", Local::now().format("%H:%M:%S%.3f"));
        for command in commands {
            if let Err(e) = store.dispatch(command) {
                println!("rejected: {e}");
            }
        }
    }
}

/// Signs `user` in through the server and the fake identity provider, without a browser window.
fn sign_in(profiles: &Profiles, guest: Store, server: &Url, user: &str) -> Store {
    let client = AppClient::Desktop { redirect_port: REDIRECT_PORT };
    let login = LoginRequest::new(server, client).unwrap_or_else(|e| fail(&e.to_string()));
    let runtime =
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap_or_else(|e| fail(&e.to_string()));
    let browser = Browser::new(server.as_str()).login_as(user);
    let callback = runtime
        .block_on(browser.authorize(login.authorize_url().as_str(), &client.redirect_uri(server)))
        .unwrap_or_else(|e| fail(&format!("the sign-in did not come back: {e}")));
    drop(runtime);
    let signed_in = login.finish(&callback).unwrap_or_else(|e| fail(&e.to_string()));
    match profiles.sign_in(&guest, signed_in) {
        Ok(SignInOutcome::Switched { store, .. }) => store,
        Ok(SignInOutcome::Renewed) => guest,
        Err(e) => fail(&e.to_string()),
    }
}

const USAGE: &str = "commands: add | toggle | quest | rename <old> => <new> | describe <t> => <text> | delete | restore \
                     | start | stop | move/due <t> => <YYYY-MM-DD|none> | repeat <t> => <day|week|month|year> [every] \
                     | norepeat | assign <t> => <quest|none> | log/retime <t> => HH:MM-HH:MM | unlog <t> \
                     | newquest | qcomplete | qreopen | qarchive | qrename <old> => <new> \
                     | qcolor <q> => <color> | qdelete \
                     | wsnew | wsswitch | wsrename <old> => <new> | wsdelete | wsrestore | show | quit";

/// The commands one input line asks for, or what is wrong with it.
fn commands_for(snapshot: &Snapshot, verb: &str, rest: &str) -> Result<Vec<Command>, String> {
    let (title, arg) = rest.split_once(" => ").map_or((rest, ""), |(t, a)| (t.trim(), a.trim()));
    let now = Utc::now();
    let task = || find_task(snapshot, title).ok_or_else(|| format!("no task {title:?}"));
    let quest = || {
        let quest = snapshot.quests.iter().find(|q| q.title == title);
        quest.map(|q| q.id).ok_or_else(|| format!("no quest {title:?}"))
    };
    let command = match verb {
        "add" => new_task(EntityId::new(), title, None),
        "quest" => return Ok(quest_with_work(title)),
        "toggle" => {
            let task = task()?;
            if task.is_done() {
                Command::ReopenTask { id: task.id }
            } else {
                Command::CompleteTask { id: task.id, at: now }
            }
        }
        "rename" => Command::RenameTask { id: task()?.id, title: arg.to_owned() },
        "describe" => Command::SetTaskDescription { id: task()?.id, description: arg.replace("\\n", "\n") },
        "delete" => Command::DeleteTask { id: task()?.id, at: now },
        "start" => Command::StartFocus { entry_id: EntityId::new(), task_id: task()?.id, at: now },
        "stop" => Command::StopFocus { at: now },
        "move" => Command::MoveTask { id: task()?.id, planned_on: day(arg)?, position: Position::Bottom, at: now },
        "due" => Command::SetTaskDue { id: task()?.id, due_on: day(arg)?, at: now },
        "repeat" => Command::SetTaskRepeat { id: task()?.id, rule: Some(rule(arg)?), now },
        "norepeat" => Command::SetTaskRepeat { id: task()?.id, rule: None, now },
        "log" => {
            let (started_at, ended_at) = times(arg)?;
            Command::LogWork { entry_id: EntityId::new(), task: WorkTask::Existing(task()?.id), started_at, ended_at }
        }
        "retime" => {
            let (started_at, ended_at) = times(arg)?;
            Command::SetEntryTimes { id: latest_session(snapshot, task()?.id)?, started_at, ended_at: Some(ended_at) }
        }
        "unlog" => Command::DeleteEntry { id: latest_session(snapshot, task()?.id)?, at: now },
        "assign" => {
            let quest_id = match arg {
                "none" => None,
                name => Some(snapshot.quests.iter().find(|q| q.title == name).ok_or(format!("no quest {name:?}"))?.id),
            };
            Command::AssignTask { id: task()?.id, quest_id }
        }
        "newquest" => Command::CreateQuest { id: EntityId::new(), title: title.to_owned(), color: QuestColor::Teal },
        "qcomplete" => Command::SetQuestStatus { id: quest()?, status: QuestStatus::Completed },
        "qreopen" => Command::SetQuestStatus { id: quest()?, status: QuestStatus::Active },
        "qarchive" => Command::SetQuestStatus { id: quest()?, status: QuestStatus::Archived },
        "qrename" => Command::RenameQuest { id: quest()?, title: arg.to_owned() },
        "qcolor" => {
            let named = QuestColor::ALL.into_iter().find(|c| format!("{c:?}").eq_ignore_ascii_case(arg));
            let color = named.ok_or_else(|| format!("no colour {arg:?}"))?;
            Command::SetQuestColor { id: quest()?, color }
        }
        "qdelete" => Command::DeleteQuest { id: quest()?, at: now },
        "wsrename" => Command::RenameWorkspace { id: workspace_named(snapshot, title, false)?, name: arg.to_owned() },
        "wsdelete" => Command::DeleteWorkspace { id: workspace_named(snapshot, title, false)?, at: now },
        "wsrestore" => Command::RestoreWorkspace { id: workspace_named(snapshot, title, true)? },
        _ => return Err(USAGE.to_owned()),
    };
    Ok(vec![command])
}

/// "2026-11-24", or "none".
fn day(arg: &str) -> Result<Option<NaiveDate>, String> {
    match arg {
        "none" => Ok(None),
        day => day.parse().map(Some).map_err(|_| format!("not a date: {day:?}")),
    }
}

/// The live (or, for `deleted`, the deleted) workspace with that name.
fn workspace_named(snapshot: &Snapshot, name: &str, deleted: bool) -> Result<EntityId, String> {
    let found = snapshot.workspaces.iter().find(|w| w.name == name && w.is_deleted() == deleted);
    found.map(|w| w.id).ok_or_else(|| format!("no workspace {name:?}"))
}

/// Today's task with that title, else any task with it.
fn find_task<'a>(snapshot: &'a Snapshot, title: &str) -> Option<&'a Task> {
    let today = Local::now().date_naive();
    snapshot.tasks_on(today).find(|t| t.title == title).or_else(|| snapshot.tasks.iter().find(|t| t.title == title))
}

/// "week 2": every second week from today, in this machine's zone (UTC if unknown).
fn rule(arg: &str) -> Result<RepeatRule, String> {
    let (unit, every) = arg.split_once(' ').unwrap_or((arg, "1"));
    let frequency = match unit {
        "day" => Frequency::Day,
        "week" => Frequency::Week,
        "month" => Frequency::Month,
        "year" => Frequency::Year,
        _ => return Err(format!("unknown frequency {unit:?}: day, week, month or year")),
    };
    let every = every.trim().parse().map_err(|_| format!("not an interval: {every:?}"))?;
    let time_zone = iana_time_zone::get_timezone().ok().and_then(|z| z.parse().ok()).unwrap_or(chrono_tz::UTC);
    Ok(RepeatRule {
        every,
        frequency,
        starts: Local::now().date_naive(),
        ends: None,
        time_zone,
        pin: RepeatPin::None,
        weekdays: Weekdays::default(),
        month_day: None,
    })
}

/// "09:00-10:30" today, local.
fn times(arg: &str) -> Result<(DateTime<Utc>, DateTime<Utc>), String> {
    let (from, to) = arg.split_once('-').ok_or_else(|| format!("expected HH:MM-HH:MM: {arg:?}"))?;
    let at = |text: &str| {
        let time = NaiveTime::parse_from_str(text.trim(), "%H:%M").map_err(|_| format!("not a time: {text:?}"))?;
        let local = Local::now().date_naive().and_time(time);
        Local
            .from_local_datetime(&local)
            .earliest()
            .map(|t| t.with_timezone(&Utc))
            .ok_or(format!("no such time: {text:?}"))
    };
    Ok((at(from)?, at(to)?))
}

/// The task's session that started last.
fn latest_session(snapshot: &Snapshot, task: EntityId) -> Result<EntityId, String> {
    let sessions = snapshot.time_entries.iter().filter(|e| e.task_id == Some(task));
    sessions.max_by_key(|e| e.started_at).map(|e| e.id).ok_or_else(|| "the task has no session".to_owned())
}

enum Event {
    Changed,
    Line(String),
}

fn new_task(id: EntityId, title: &str, quest_id: Option<EntityId>) -> Command {
    Command::CreateTask {
        id,
        title: title.to_owned(),
        quest_id,
        planned_on: Some(Local::now().date_naive()),
        due_on: None,
        position: Position::Bottom,
    }
}

fn quest_with_work(title: &str) -> Vec<Command> {
    let (quest, task) = (EntityId::new(), EntityId::new());
    let now = Utc::now();
    vec![
        Command::CreateQuest { id: quest, title: title.to_owned(), color: QuestColor::Blue },
        new_task(task, &format!("Work on {title}"), Some(quest)),
        Command::LogTime {
            entry_id: EntityId::new(),
            task_id: Some(task),
            started_at: now - chrono::Duration::minutes(90),
            ended_at: now,
        },
    ]
}

fn describe(status: &SyncStatus, snapshot: &Snapshot) -> String {
    let status = match status {
        SyncStatus::SignedOut => "signed out".to_owned(),
        SyncStatus::Connecting => "connecting".to_owned(),
        SyncStatus::Live { pending } => format!("live, {pending} pending"),
        SyncStatus::Offline { pending, .. } => format!("offline, {pending} pending"),
        SyncStatus::Failed { error } => format!("failed: {error}"),
    };
    let today = Local::now().date_naive();
    let tasks: Vec<String> = snapshot.tasks_on(today).map(|t| task_line(snapshot, t)).collect();
    let mut sessions: Vec<_> =
        snapshot.time_entries.iter().filter(|e| e.started_at.with_timezone(&Local).date_naive() == today).collect();
    sessions.sort_by_key(|e| e.started_at);
    let sessions: Vec<String> = sessions.into_iter().map(|e| session_line(snapshot, e)).collect();
    let quests: Vec<String> = snapshot.quests.iter().map(quest_line).collect();
    let list = |items: Vec<String>| if items.is_empty() { "-".to_owned() } else { items.join(", ") };
    let shown = snapshot.workspace(snapshot.workspace).map_or("?", |w| w.name.as_str());
    let others: Vec<String> = snapshot
        .workspaces
        .iter()
        .filter(|w| w.id != snapshot.workspace)
        .map(|w| if w.is_deleted() { format!("{} (deleted)", w.name) } else { w.name.clone() })
        .collect();
    format!(
        "{status} | workspace: {shown} (also: {}) | today: {} | sessions: {} | quests: {}",
        list(others),
        list(tasks),
        list(sessions),
        list(quests)
    )
}

/// "Garden (done, green)" with the parts that apply.
fn quest_line(quest: &Quest) -> String {
    let mut parts = vec![format!("{:?}", quest.color).to_lowercase()];
    match quest.status {
        QuestStatus::Completed => parts.insert(0, "done".to_owned()),
        QuestStatus::Archived => parts.insert(0, "archived".to_owned()),
        QuestStatus::Active => {}
    }
    format!("{} ({})", quest.title, parts.join(", "))
}

/// "[x] Title (Quest) !2026-11-27 ▶ “notes” ↻week" with the parts that apply.
fn task_line(snapshot: &Snapshot, task: &Task) -> String {
    let mut line = format!("[{}] {}", if task.is_done() { "x" } else { " " }, task.title);
    if let Some(quest) = task.quest_id.and_then(|id| snapshot.quest(id)) {
        line.push_str(&format!(" ({})", quest.title));
    }
    if let Some(due) = task.due_on {
        line.push_str(&format!(" !{due}"));
    }
    if snapshot.running_entry().is_some_and(|e| e.task_id == Some(task.id)) {
        line.push_str(" ▶");
    }
    if !task.description.is_empty() {
        line.push_str(&format!(" “{}”", task.description.replace('\n', "⏎")));
    }
    let series = task.series.and_then(|id| snapshot.task(id)).unwrap_or(task);
    if let Some(schedule) = series.repeat {
        line.push_str(&format!(" ↻{}x{:?}", schedule.rule.every, schedule.rule.frequency));
    }
    line
}

/// "09:00–10:30 Draft (Book)"; a running one ends in "…".
fn session_line(snapshot: &Snapshot, entry: &TimeEntry) -> String {
    let clock = |at: DateTime<Utc>| at.with_timezone(&Local).format("%H:%M").to_string();
    let end = entry.ended_at.map_or_else(|| "…".to_owned(), clock);
    let untitled = if entry.kind == EntryKind::Organizing { "Organizing" } else { "Other work" };
    let title = entry.task_id.and_then(|id| snapshot.task(id)).map_or(untitled, |t| t.title.as_str());
    let quest = entry.quest_id.and_then(|id| snapshot.quest(id)).map(|q| format!(" ({})", q.title)).unwrap_or_default();
    format!("{}–{end} {title}{quest}", clock(entry.started_at))
}

fn fail(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(1);
}
