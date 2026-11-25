//! The app's own command-line flags. Rune takes its flags (`--snap`, `--tree`, …) first and
//! hands the rest over as `rune::extra_args()`.

use std::path::PathBuf;

use chrono::NaiveDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Where the local database lives: `--data-dir`, else `QUEST_DATA_DIR`, else the default.
    pub data_dir: PathBuf,
    /// Seed demo quests, tasks and tracked time into an empty store.
    pub demo: bool,
    /// Pin the clock to a local time (`--now 2026-10-15T14:40`), for reproducible snapshots.
    pub now: Option<NaiveDateTime>,
    /// AI capture answers with this proposal (a `CaptureProposal` as JSON) instead of asking the
    /// server: for the E2E run and demos without network or AI provider.
    pub capture_fixture: Option<PathBuf>,
    /// Development and E2E only: keep sign-in tokens in this plain file (0600) instead of the
    /// macOS keychain.
    pub token_file: Option<PathBuf>,
    /// Development and E2E only: open sign-in pages with this program (it gets the URL) instead
    /// of the system browser.
    pub browser: Option<PathBuf>,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum CliError {
    #[error("{flag} needs a value, for example `{flag} {example}`")]
    MissingValue { flag: &'static str, example: &'static str },
    #[error("`--now {value}` is not a local time like 2026-10-15T14:40")]
    BadTime { value: String },
    #[error(
        "unknown argument `{0}`. Quest takes --data-dir <dir>, --demo, --now <time>, --capture-fixture <file>, --token-file <file> and --browser <program>, plus Rune's flags (--snap, --tree, --scene …)"
    )]
    Unknown(String),
}

/// Parses the app's flags; `env_data_dir` is `QUEST_DATA_DIR` and `home` the user's home folder.
pub fn parse(args: &[String], env_data_dir: Option<PathBuf>, home: Option<PathBuf>) -> Result<Options, CliError> {
    let mut data_dir = None;
    let mut demo = false;
    let mut now = None;
    let mut capture_fixture = None;
    let mut token_file = None;
    let mut browser = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--demo" => demo = true,
            "--data-dir" => {
                let value =
                    rest.next().ok_or(CliError::MissingValue { flag: "--data-dir", example: "/tmp/quest-a" })?;
                data_dir = Some(PathBuf::from(value));
            }
            "--now" => {
                let value = rest.next().ok_or(CliError::MissingValue { flag: "--now", example: "2026-10-15T14:40" })?;
                now = Some(parse_time(value)?);
            }
            "--capture-fixture" => {
                let value = rest
                    .next()
                    .ok_or(CliError::MissingValue { flag: "--capture-fixture", example: "proposal.json" })?;
                capture_fixture = Some(PathBuf::from(value));
            }
            "--token-file" => {
                let value = rest
                    .next()
                    .ok_or(CliError::MissingValue { flag: "--token-file", example: "/tmp/quest-a/tokens.json" })?;
                token_file = Some(PathBuf::from(value));
            }
            "--browser" => {
                let value =
                    rest.next().ok_or(CliError::MissingValue { flag: "--browser", example: "e2e/browser.py" })?;
                browser = Some(PathBuf::from(value));
            }
            other => return Err(CliError::Unknown(other.to_owned())),
        }
    }
    let data_dir = data_dir.or(env_data_dir).unwrap_or_else(|| default_data_dir(home));
    Ok(Options { data_dir, demo, now, capture_fixture, token_file, browser })
}

fn parse_time(value: &str) -> Result<NaiveDateTime, CliError> {
    ["%Y-%m-%dT%H:%M", "%Y-%m-%dT%H:%M:%S"]
        .iter()
        .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
        .ok_or_else(|| CliError::BadTime { value: value.to_owned() })
}

fn default_data_dir(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from(".")).join("Library/Application Support/Quest")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn defaults_to_application_support() {
        let options = parse(&[], None, Some(PathBuf::from("/home/me"))).unwrap();
        assert_eq!(options.data_dir, PathBuf::from("/home/me/Library/Application Support/Quest"));
        assert!(!options.demo);
    }

    #[test]
    fn flag_beats_environment() {
        let options = parse(&args(&["--data-dir", "/tmp/a", "--demo"]), Some("/tmp/env".into()), None).unwrap();
        assert_eq!(options.data_dir, PathBuf::from("/tmp/a"));
        assert!(options.demo);
        let options = parse(&[], Some("/tmp/env".into()), None).unwrap();
        assert_eq!(options.data_dir, PathBuf::from("/tmp/env"));
    }

    #[test]
    fn parses_now_and_rejects_garbage() {
        let options = parse(&args(&["--now", "2026-10-15T14:40"]), None, None).unwrap();
        assert_eq!(options.now.unwrap().to_string(), "2026-10-15 14:40:00");
        assert!(matches!(parse(&args(&["--now", "soon"]), None, None), Err(CliError::BadTime { .. })));
        assert!(matches!(parse(&args(&["--data-dir"]), None, None), Err(CliError::MissingValue { .. })));
        assert!(matches!(parse(&args(&["--wat"]), None, None), Err(CliError::Unknown(_))));
        let options = parse(&args(&["--capture-fixture", "p.json"]), None, None).unwrap();
        assert_eq!(options.capture_fixture, Some(PathBuf::from("p.json")));
        let options = parse(&args(&["--token-file", "t.json", "--browser", "b.py"]), None, None).unwrap();
        assert_eq!(options.token_file, Some(PathBuf::from("t.json")));
        assert_eq!(options.browser, Some(PathBuf::from("b.py")));
        assert!(matches!(parse(&args(&["--browser"]), None, None), Err(CliError::MissingValue { .. })));
    }
}
