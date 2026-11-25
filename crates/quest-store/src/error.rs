/// Everything the store reports to the app. Messages say what failed and what to do next.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    #[error(
        "could not use the local database at {path}: {message}. Check that the folder exists, is writable and the disk is not full."
    )]
    Storage { path: String, message: String },
    #[error("could not use the token store: {message}. Sign in again; if it keeps failing, restart the app.")]
    TokenStore { message: String },
    #[error("your sign-in has expired. Sign in again; your local data stays.")]
    SessionExpired,
    #[error("signing in was cancelled.")]
    SignInCancelled,
    #[error("signing in did not complete: {reason}. Try again.")]
    SignInFailed { reason: String },
    #[error(
        "you signed in as {signing_in}, but {active} is signed in on this device. Sign out first, or sign in with that account."
    )]
    AnotherAccountActive { active: String, signing_in: String },
    #[error("this needs an account. Sign in first.")]
    NotSignedIn,
    #[error("the server URL {url} is not usable: {message}. Enter an http:// or https:// address.")]
    InvalidServerUrl { url: String, message: String },
    #[error("could not reach the server at {server_url}: {message}. Check the URL and your connection.")]
    Unreachable { server_url: String, message: String },
    #[error("the server rejected the request: {message}")]
    Server { message: String },
    #[error("the server speaks a newer sync protocol. Update the app to keep syncing.")]
    UnsupportedProtocol,
    #[error("the background worker stopped unexpectedly; changes are no longer saved. Restart the app.")]
    WorkerStopped,
}
