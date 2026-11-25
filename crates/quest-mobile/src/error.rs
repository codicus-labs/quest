/// Errors the native UI can show. Messages say what failed and what to do. Flat: foreign code
/// gets the variant and the display message (fields named `message` would also clash with
/// `Throwable.message` in Kotlin).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum MobileError {
    #[error("{detail}")]
    Storage { detail: String },
    #[error("unknown time zone {name:?}. Pass an IANA name such as \"Europe/Berlin\".")]
    InvalidTimeZone { name: String },
    #[error("the time {epoch_ms} ms since 1970 is out of range. Pass the current time from the system clock.")]
    InvalidMoment { epoch_ms: i64 },
    #[error("{value:?} is not a valid id. Use the id from the last home data.")]
    InvalidId { value: String },
    #[error("the server URL {value:?} is not valid: {reason}. Enter an http:// or https:// address.")]
    InvalidServerUrl { value: String, reason: String },
    #[error(
        "the server URL {value:?} is not encrypted. Use an https:// address; http:// is only allowed for this \
         device (localhost, 127.0.0.1, ::1)."
    )]
    InsecureServerUrl { value: String },
    /// Carries no value: it would show the password.
    #[error(
        "the server URL contains a user name or password. Remove the part before \"@\"; you sign in in the \
         browser instead."
    )]
    ServerUrlWithCredentials,
    #[error("a weekly target of {minutes} minutes is out of range. Choose between 1 and 168 hours.")]
    InvalidWeeklyTarget { minutes: u32 },
    #[error("the task {id:?} no longer exists. Close it and pick a task from the current list.")]
    UnknownTask { id: String },
    #[error("the task {id:?} is done. Reopen it before starting its timer.")]
    TaskIsDone { id: String },
    #[error("the title is empty. Type a title first.")]
    EmptyTitle,
    #[error("{value:?} is not a date. Pass a day like \"2026-11-24\".")]
    InvalidDate { value: String },
    /// `reason` is the desktop's message for the field to correct, e.g. "Enter a start date.".
    #[error("{reason}")]
    InvalidRepeat { reason: String },
    #[error("the quest {id:?} no longer exists. It may be in the archive; go back to the quests.")]
    UnknownQuest { id: String },
    #[error("the time entry {id:?} no longer exists. Close the editor; the calendar shows the current entries.")]
    UnknownEntry { id: String },
    /// A form field to correct; `reason` is the desktop's message, e.g. "The end must be after the start.".
    #[error("{reason}")]
    InvalidForm { reason: String },
    #[error("{reason}")]
    Rejected { reason: String },
    #[error("AI capture needs an account. Sign in to your Quest server in Account first.")]
    AiSignedOut,
    #[error("AI capture is not set up on your Quest server. Ask its admin to configure an AI provider.")]
    AiNotConfigured,
    /// `detail` says what failed on the way to the server (AI capture, connected AI clients).
    #[error("could not reach your Quest server ({detail}). Check your connection and try again.")]
    ServerUnreachable { detail: String },
    #[error("the AI provider is not available right now. Try again in a moment.")]
    AiUnavailable,
    #[error("too many AI requests. Wait a minute and try again.")]
    AiRateLimited,
    #[error("the AI answer could not be used. Try again, maybe with other words.")]
    AiInvalidResponse,
    #[error("the text is too long for AI capture. Shorten it to at most {max_chars} characters.")]
    CaptureTextTooLong { max_chars: u32 },
    #[error("this needs an account. Sign in to your Quest server in Account first.")]
    NotSignedIn,
    #[error("your sign-in has expired. Sign in again in Account; your local data stays.")]
    SessionExpired,
    #[error("AI features are off for your account. Turn them on in Settings.")]
    AiDisabled,
    #[error("signing in was cancelled.")]
    SignInCancelled,
    /// `reason` is the store's message, saying what to do.
    #[error("{reason}")]
    SignInFailed { reason: String },
    #[error("this sign-in is no longer waiting (Quest was restarted meanwhile). Tap Sign in again.")]
    SignInExpired,
    #[error("signing out is under way, so this change was not made. It can be made again as a guest.")]
    SigningOut,
    #[error("this AI client is no longer connected. The list shows the current ones.")]
    UnknownConnection,
    #[error("internal error: {detail}. Please report this; restarting the app may help.")]
    Internal { detail: String },
}
