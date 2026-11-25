//! The pages of `/authorize` (consent, "Continue as …"), sign-in errors and sign-out. Plain HTML,
//! no scripts.

use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, REFERRER_POLICY, X_FRAME_OPTIONS};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use super::policy::Access;

/// What the consent page shows, plus the request it re-submits.
pub struct Consent<'a> {
    pub client_name: &'a str,
    pub redirect_host: &'a str,
    /// Every redirect URI is on localhost: anyone could claim this client's name.
    pub loopback_only: bool,
    /// Hidden fields: the authorization request's parameters.
    pub hidden: &'a [(&'static str, &'a str)],
    /// Who is signed in, e.g. "Anna (anna@example.com)".
    pub account: &'a str,
    pub csrf: &'a str,
    /// Preselected: what the client asked for.
    pub access: Access,
}

const STYLE: &str = "body{font:16px/1.5 -apple-system,system-ui,sans-serif;max-width:26rem;margin:3rem auto;padding:0 1rem;color:#1d1d1f}\
h1{font-size:1.4rem}fieldset{border:1px solid #bbb;border-radius:8px;margin-top:1rem}label{display:block;padding:.4rem 0;min-height:32px}\
.row{display:flex;gap:.6rem;margin-top:1.2rem}button{flex:1;padding:.7rem;font-size:1rem;border-radius:8px;border:1px solid #bbb;background:#fff;min-height:48px}\
button.primary,a.primary{background:#1d1d1f;color:#fff;border-color:#1d1d1f}\
a.button{flex:1;display:flex;align-items:center;justify-content:center;min-height:48px;border-radius:8px;border:1px solid;text-decoration:none}button.link{background:none;border:none;text-decoration:underline;flex:none;padding:0;min-height:48px}\
.warn{background:#fff4e0;border-radius:8px;padding:.6rem}.error{color:#b00020;font-weight:600}ul{padding-left:1.2rem}\
@media(prefers-color-scheme:dark){body{background:#1c1c1e;color:#f2f2f7}\
button{background:#2c2c2e;color:#f2f2f7;border-color:#555}button.primary,a.primary{background:#f2f2f7;color:#1c1c1e}button.link{background:none}.warn{background:#3a2e14}}";

pub fn consent(page: &Consent) -> Response {
    let client = escape(page.client_name);
    let host = escape(page.redirect_host);
    let warning = if page.loopback_only {
        "<p class=\"warn\">This client only returns to a program on this computer (localhost). Its name is not \
         verified: connect only if you just started this yourself.</p>"
    } else {
        ""
    };
    let checked = |access: Access| if page.access == access { " checked" } else { "" };
    let body = format!(
        "<h1>Connect {client} to Quest?</h1>\
         <p>Signed in as <strong>{account}</strong>. After you allow it, you are sent back to \
         <strong>{host}</strong>.</p>{warning}\
         <form method=\"post\" action=\"/authorize\">{hidden}\
         <fieldset><legend>{client} may</legend>\
         <label><input type=\"radio\" name=\"access\" value=\"quest:read\"{read}> \
         <strong>Read only</strong>: see your quests, tasks and tracked time.</label>\
         <label><input type=\"radio\" name=\"access\" value=\"quest:write\"{write}> \
         <strong>Read and write</strong>: also change them and start or stop your timer.</label></fieldset>\
         <ul><li>Deleting only archives; everything can be restored.</li>\
         <li>You can disconnect it any time under Settings → Connected AI clients.</li></ul>\
         <div class=\"row\"><button type=\"submit\" name=\"decision\" value=\"deny\">Cancel</button>\
         <button class=\"primary\" type=\"submit\" name=\"decision\" value=\"allow\">Allow</button></div>\
         <p>Not you? <button class=\"link\" type=\"submit\" name=\"decision\" value=\"switch\">Use another account</button></p>\
         </form>",
        account = escape(page.account),
        hidden = hidden_fields(page.hidden, page.csrf),
        read = checked(Access::Read),
        write = checked(Access::Write),
    );
    html(StatusCode::OK, "Connect to Quest", &body)
}

/// A Quest app asks to sign in while the browser already has a session.
pub fn continue_as(account: &str, csrf: &str, hidden: &[(&'static str, &str)]) -> Response {
    let body = format!(
        "<h1>Sign in to the Quest app</h1><p>You are signed in as <strong>{account}</strong>.</p>\
         <form method=\"post\" action=\"/authorize\">{hidden}\
         <div class=\"row\"><button type=\"submit\" name=\"decision\" value=\"deny\">Cancel</button>\
         <button class=\"primary\" type=\"submit\" name=\"decision\" value=\"allow\">Continue</button></div>\
         <p>Not you? <button class=\"link\" type=\"submit\" name=\"decision\" value=\"switch\">Use another account</button></p>\
         </form>",
        account = escape(account),
        hidden = hidden_fields(hidden, csrf),
    );
    html(StatusCode::OK, "Sign in to Quest", &body)
}

/// A page with a title and one paragraph, e.g. a failed sign-in or "Signed out".
pub fn message(status: StatusCode, title: &str, text: &str) -> Response {
    let body = format!("<h1>{}</h1><p>{}</p>", escape(title), escape(text));
    html(status, "Quest", &body)
}

/// `GET /auth/android` when Android did not hand the App Link to the app. The empty `href` reopens
/// this very URL (with the code) on a tap: a user gesture, so Android can now open the app, and the
/// page never has to write the code itself.
pub fn app_link_fallback() -> Response {
    let body = "<h1>Open Quest</h1><p>Open Quest to finish signing in.</p>\
         <div class=\"row\"><a class=\"button primary\" href=\"\">Open Quest</a></div>\
         <p>If the app still does not open, close this window and sign in again in the app.</p>";
    html(StatusCode::OK, "Quest", body)
}

fn hidden_fields(fields: &[(&'static str, &str)], csrf: &str) -> String {
    let mut html: String = fields
        .iter()
        .map(|(name, value)| format!("<input type=\"hidden\" name=\"{name}\" value=\"{}\">", escape(value)))
        .collect();
    html.push_str(&format!("<input type=\"hidden\" name=\"csrf\" value=\"{}\">", escape(csrf)));
    html
}

/// A request that cannot be sent back to the client (unknown client or redirect URI).
pub fn error(status: StatusCode, message: &str) -> Response {
    let body = format!(
        "<h1>This connection request cannot be used</h1><p class=\"error\">{}</p>\
         <p>Start connecting again from your AI client. If it keeps failing, the client may not be set up for this server.</p>",
        escape(message)
    );
    html(status, "Quest", &body)
}

/// 429 for too many sign-ins or page views from one address, e.g. `what` "Too many requests" and
/// `wait` "5 minutes".
pub fn rate_limited(what: &str, wait: &str) -> Response {
    let (what, wait) = (escape(what), escape(wait));
    let body =
        format!("<h1>{what}</h1><p class=\"error\">{what} from your network. Wait {wait}, then reload this page.</p>");
    html(StatusCode::TOO_MANY_REQUESTS, "Quest", &body)
}

fn html(status: StatusCode, title: &str, body: &str) -> Response {
    let document = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title}</title>\
         <style>{STYLE}</style></head><body>{body}</body></html>"
    );
    let mut response = (status, document).into_response();
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'"),
    );
    response
}

pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
