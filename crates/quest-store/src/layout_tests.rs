use super::*;
use crate::db::Db;

fn url(raw: &str) -> Url {
    raw.parse().unwrap()
}

#[test]
fn the_key_depends_on_server_issuer_and_subject() {
    let key = AccountKey::new(&url("https://quest.example.com"), "https://id.example", "anna");
    assert_eq!(key.as_str().len(), 32);
    assert!(key.as_str().bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(key, AccountKey::new(&url("https://quest.example.com/"), "https://id.example", "anna"));
    assert_eq!(key, AccountKey::new(&url("https://quest.example.com/some/path"), "https://id.example", "anna"));
    assert_ne!(key, AccountKey::new(&url("https://other.example.com"), "https://id.example", "anna"));
    assert_ne!(key, AccountKey::new(&url("https://quest.example.com"), "https://id2.example", "anna"));
    assert_ne!(key, AccountKey::new(&url("https://quest.example.com"), "https://id.example", "ben"));
    // The separator keeps the parts apart.
    assert_ne!(
        AccountKey::new(&url("https://q.example"), "a\nb", "c"),
        AccountKey::new(&url("https://q.example"), "a", "b\nc")
    );
}

#[test]
fn only_well_formed_keys_are_read_back() {
    let key = AccountKey::new(&url("https://quest.example.com"), "i", "s");
    let json = serde_json::to_string(&key).unwrap();
    assert_eq!(serde_json::from_str::<AccountKey>(&json).unwrap(), key);
    for bad in [r#""../../etc""#, r#""ABCDEF00000000000000000000000000""#, r#""abc""#] {
        assert!(serde_json::from_str::<AccountKey>(bad).is_err(), "{bad}");
    }
}

#[test]
fn the_active_profile_defaults_to_guest_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let layout = Layout::new(dir.path().join("data"));
    assert_eq!(layout.read_active().unwrap(), ActiveProfile::Guest);
    let account = ActiveProfile::Account {
        server_url: url("http://127.0.0.1:8787"),
        client_id: "quest-desktop".into(),
        key: AccountKey::new(&url("http://127.0.0.1:8787"), "i", "s"),
    };
    layout.write_active(&account).unwrap();
    assert_eq!(layout.read_active().unwrap(), account);
    layout.write_active(&ActiveProfile::Guest).unwrap();
    assert_eq!(layout.read_active().unwrap(), ActiveProfile::Guest);
}

#[test]
fn an_unreadable_active_file_is_an_error_not_a_guest() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("active.json"), "{nonsense").unwrap();
    assert!(matches!(Layout::new(dir.path().into()).read_active(), Err(StoreError::Storage { .. })));
}

#[test]
fn removing_an_account_deletes_its_folder() {
    let dir = tempfile::tempdir().unwrap();
    let layout = Layout::new(dir.path().into());
    let key = AccountKey::new(&url("http://127.0.0.1:1"), "i", "s");
    Db::open(&layout.account_dir(&key)).unwrap();
    layout.remove_account_dir(&key).unwrap();
    assert!(!layout.account_dir(&key).exists());
    layout.remove_account_dir(&key).unwrap();
    layout.remove_leftovers().unwrap();
    assert!(!dir.path().join("removed").exists());
}
