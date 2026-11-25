use super::*;

fn key(subject: &str) -> AccountKey {
    AccountKey::new(&"http://127.0.0.1:1".parse().unwrap(), "https://id.example", subject)
}

fn tokens(access: &str) -> StoredTokens {
    StoredTokens { access_token: access.into(), refresh_token: Some("refresh".into()), access_expires_at_ms: 42 }
}

fn round_trip(vault: &dyn TokenVault) {
    assert_eq!(vault.load(&key("anna")).unwrap(), None);
    vault.save(&key("anna"), &tokens("a1")).unwrap();
    vault.save(&key("ben"), &tokens("b1")).unwrap();
    vault.save(&key("anna"), &tokens("a2")).unwrap();
    assert_eq!(vault.load(&key("anna")).unwrap(), Some(tokens("a2")));
    vault.delete(&key("anna")).unwrap();
    vault.delete(&key("anna")).unwrap();
    assert_eq!(vault.load(&key("anna")).unwrap(), None);
    assert_eq!(vault.load(&key("ben")).unwrap(), Some(tokens("b1")));
}

#[test]
fn memory_vault_keeps_tokens_per_account() {
    round_trip(&MemoryVault::default());
}

#[test]
fn file_vault_keeps_tokens_per_account_across_instances() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tokens.json");
    round_trip(&FileVault::new(&path));
    assert_eq!(FileVault::new(&path).load(&key("ben")).unwrap(), Some(tokens("b1")));
}

#[cfg(unix)]
#[test]
fn the_token_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tokens.json");
    FileVault::new(&path).save(&key("anna"), &tokens("a")).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn debug_output_hides_tokens() {
    let shown = format!("{:?}", tokens("secret-access"));
    assert!(!shown.contains("secret-access") && !shown.contains("\"refresh\""), "{shown}");
}
