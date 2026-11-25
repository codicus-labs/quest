use std::sync::Arc;

use super::*;
use crate::core::ChangeListener;
use crate::test_support::moment;
use crate::types::{HomeQuery, SyncOutcome};

struct Quiet;

impl ChangeListener for Quiet {
    fn on_change(&self) {}
}

fn open(dir: &tempfile::TempDir) -> Arc<QuestCore> {
    QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), Arc::new(Quiet)).unwrap()
}

/// Wednesday, 25 Nov 2026, 10:00 in Berlin.
fn now() -> Moment {
    moment("2026-11-25T10:00:00+01:00")
}

#[test]
fn a_quick_add_shows_on_home_with_its_quest_and_description() {
    let dir = tempfile::tempdir().unwrap();
    let core = open(&dir);
    let quest = core.create_quest("Reading".into(), None).unwrap();
    let form = core.quick_add_form(Some("A good read\nhttps://example.com".into()), now()).unwrap();
    assert_eq!(form.quest.as_ref().map(|q| q.id.as_str()), Some(quest.as_str()));

    let id =
        core.quick_add(form.title, form.description, form.quest.map(|q| q.id), Some(form.today.iso), now()).unwrap();
    let home = core.home(now(), HomeQuery::default()).unwrap();
    assert_eq!(home.quest_work.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), [id.as_str()]);
    assert_eq!(core.task_detail(id, now()).unwrap().description, "https://example.com");
}

#[test]
fn quick_draft_preview_and_creation_share_defaults_and_token_order() {
    use crate::{QuickAddContext, quick_add_defaults};
    let dir = tempfile::tempdir().unwrap();
    let core = open(&dir);
    let quest = core.create_quest("Reading".into(), None).unwrap();
    let defaults = quick_add_defaults(QuickAddContext::Inbox, now()).unwrap();
    let text = "Read two pages #read @fri !tom";
    let preview = core.preview_quick_add(text.into(), defaults.clone(), now()).unwrap();
    assert_eq!(preview.title, "Read two pages");
    assert_eq!(preview.fields.quest_id.as_deref(), Some(quest.as_str()));
    assert_eq!(preview.fields.planned_on.as_deref(), Some("2026-11-26"));
    assert_eq!(preview.fields.due_on, preview.fields.planned_on);
    let id = core.add_quick_draft(text.into(), String::new(), defaults, now()).unwrap();
    let task = core.task_detail(id, now()).unwrap();
    assert_eq!(task.quest.unwrap().id, quest);
    assert_eq!(task.planned_on.map(|d| d.iso), preview.fields.planned_on);
    assert_eq!(task.due_on.map(|d| d.iso), preview.fields.due_on);
}

#[test]
fn flushed_changes_survive_a_kill() {
    let dir = tempfile::tempdir().unwrap();
    let core = open(&dir);
    let id = core.quick_add("Before the kill".into(), String::new(), None, None, now()).unwrap();
    assert!(core.flush(10_000));
    // A killed process never drops the core.
    std::mem::forget(core);
    let reopened = open(&dir);
    assert_eq!(reopened.task_detail(id, now()).unwrap().title, "Before the kill");
}

#[test]
fn a_signed_out_core_is_offline_at_once() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(open(&dir).sync_until_idle(10_000), SyncOutcome::Offline);
}

#[test]
fn a_picked_day_gets_its_label() {
    let day = picked_day("2026-11-26".into()).unwrap();
    assert_eq!((day.iso.as_str(), day.label.as_str()), ("2026-11-26", "Nov 26, 2026"));
    assert_eq!(picked_day("soon".into()), Err(MobileError::InvalidDate { value: "soon".into() }));
}
