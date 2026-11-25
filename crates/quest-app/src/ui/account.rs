//! The account dialog: sign in (in the browser) to a server, see who is signed in and the sync
//! status, sign out, switch the theme and AI features, and see or disconnect the AI clients
//! connected over MCP.

use quest_store::{Account, AiConnection};
use rune::{El, TextEdit, Tx, col, row, spacer, text, text_input};
use rune_kit::{Button, ButtonVariant, Dialog, Switch};

use crate::state::ai_connections::{ConnectionsView, connection_detail, connection_name};
use crate::state::auth::session_expired;
use crate::state::{AuthStep, Overlay, State};
use crate::theme::{Pal, ty};
use crate::ui::chrome::{sync_badge, tone_color};
use crate::ui::parts;
use crate::{clock, icons};

pub fn layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let k = &p.kit;
    let (tone, status) = sync_badge(&s.sync);
    let status_row = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .py(4.0)
        .child(parts::dot(tone_color(tone, p), 8.0))
        .child(text(status).key("account-status").font(ty::SMALL).color(p.muted).grow(1.0));
    let theme_row =
        row().w_full().center_y().pt(8.0).child(text("Appearance").font(ty::BODY).color(p.text)).child(spacer()).child(
            parts::button(
                "account-theme",
                if s.dark { "Light theme" } else { "Dark theme" },
                Some(if s.dark { &icons::SUN } else { &icons::MOON }),
                p,
                |s: &mut State, _: &mut Tx| s.dark = !s.dark,
            )
            .h(32.0),
        );
    let mut body = col().w_full().gap(12.0).px(16.0).pb(16.0).child(status_row);
    if s.rejected > 0 {
        body = body.child(
            text(format!(
                "{} change(s) were refused by the server. They stay on this Mac but won’t reach other devices.",
                s.rejected
            ))
            .key("account-rejected")
            .font(ty::SMALL)
            .color(p.accent)
            .wrap(),
        );
    }
    if let Some(notice) = &s.notice {
        body =
            body.child(text(notice.clone()).key("account-notice").role("alert").font(ty::SMALL).color(p.accent).wrap());
    }
    body = match &s.account {
        Some(account) => signed_in(s, account, body, p),
        None => guest(s, body, p),
    };
    body = body.child(theme_row).child(ai_features_row(s, p));
    if s.account.is_some() {
        body = body.child(ai_clients(s, p));
    }
    Dialog::new("account", "Account & sync")
        .description("Quest works offline. Sign in to sync between devices.")
        .open(true)
        .fit()
        .width(420.0)
        .on_close(|s: &mut State| {
            // Signing out keeps the dialog: edits behind it would be deleted with the account.
            if !s.auth.locks_edits() {
                s.overlay = Overlay::None;
            }
        })
        .child(body)
        .build(k, layer);
}

/// The signed-in account: who, where, "Sign in again" after the session expired, and signing out
/// (with a warning when changes haven't reached the server).
fn signed_in(s: &State, account: &Account, body: El, p: &Pal) -> El {
    let k = &p.kit;
    let info = &account.info;
    let name = info.name.clone().or_else(|| info.email.clone()).unwrap_or_else(|| info.subject.clone());
    let mut body = body.child(text(format!("Signed in as {name}")).key("account-name").font(ty::BODY).color(p.text));
    if let Some(email) = info.email.as_ref().filter(|_| info.name.is_some()) {
        body = body.child(text(email.clone()).key("account-email").font(ty::SMALL).color(p.muted));
    }
    body = body.child(text(account.server_url.to_string()).font(ty::SMALL).color(p.faint));
    if session_expired(&s.sync) {
        body = sign_in_controls(s, body, "Sign in again", "account-sign-in-again", p);
    }
    match s.auth.step {
        AuthStep::ConfirmSignOut { pending } => body
            .child(
                text(format!(
                    "{pending} change(s) haven’t reached the server yet. Signing out deletes them from this Mac."
                ))
                .key("account-sign-out-warning")
                .role("alert")
                .font(ty::SMALL)
                .color(p.accent)
                .wrap(),
            )
            .child(
                row()
                    .gap(8.0)
                    .child(
                        Button::new("Keep signed in")
                            .variant(ButtonVariant::Outline)
                            .on_press(|s: &mut State| s.keep_signed_in())
                            .build(k)
                            .key("account-keep-signed-in"),
                    )
                    .child(
                        Button::new("Sign out anyway")
                            .variant(ButtonVariant::Contrast)
                            .on_press(|s: &mut State| s.sign_out())
                            .build(k)
                            .key("account-sign-out-confirm"),
                    ),
            ),
        AuthStep::SigningOut => body.child(note("account-progress", "Signing out… Editing waits until it is done.", p)),
        AuthStep::Importing => body.child(note("account-progress", "Importing the guest data…", p)),
        _ => body.child(
            Button::new("Sign out")
                .variant(ButtonVariant::Outline)
                .on_press(|s: &mut State| s.ask_sign_out())
                .build(k)
                .key("account-sign-out"),
        ),
    }
}

/// The guest: the server to sign in to, and "Sign in" (in the browser).
fn guest(s: &State, body: El, p: &Pal) -> El {
    let body = body.child(input("account-server", &s.account_form.server, "Server URL", p, |s: &mut State| {
        &mut s.account_form.server
    }));
    sign_in_controls(s, body, "Sign in", "account-sign-in", p)
}

/// "Sign in" (or "Sign in again"); while the browser is open, a note and "Cancel".
fn sign_in_controls(s: &State, body: El, label: &str, key: &str, p: &Pal) -> El {
    match s.auth.step {
        AuthStep::SigningIn { .. } => body
            .child(note("account-progress", "Continue in your browser. Quest waits for the sign-in to finish.", p))
            .child(
                Button::new("Cancel")
                    .variant(ButtonVariant::Outline)
                    .on_press(|s: &mut State| s.cancel_sign_in())
                    .build(&p.kit)
                    .key("account-sign-in-cancel"),
            ),
        AuthStep::CancellingSignIn => body.child(note("account-progress", "Cancelling…", p)),
        _ => body.child(
            Button::new(label)
                .variant(ButtonVariant::Contrast)
                .on_press(|s: &mut State| s.start_sign_in())
                .build(&p.kit)
                .key(key.to_owned()),
        ),
    }
}

fn note(key: &str, message: &str, p: &Pal) -> El {
    text(message.to_owned()).key(key.to_owned()).font(ty::SMALL).color(p.muted).wrap()
}

/// The account's "AI features" setting (kept on the server, for all devices). Off by default;
/// turning it on shows the privacy notice first.
fn ai_features_row(s: &State, p: &Pal) -> El {
    let (on, usable, detail) = match &s.account {
        None => (false, false, "Sign in to use AI features."),
        Some(account) if !account.info.ai_available => {
            (false, false, "Your Quest server has no AI provider configured.")
        }
        Some(account) => (
            account.info.ai_enabled,
            !matches!(s.auth.step, AuthStep::SavingAi),
            "Capture with AI (⌘K) structures free text with your Quest server’s AI provider. Applies on all your devices.",
        ),
    };
    let switch = Switch::new(on)
        .label("AI features")
        .hide_label()
        .disabled(!usable)
        .on_toggle(|s: &mut State| s.toggle_ai_features());
    row()
        .w_full()
        .gap(12.0)
        .center_y()
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(text("AI features").font(ty::BODY).color(p.text))
                .child(text(detail).key("account-ai-detail").font(ty::SMALL).color(p.faint).wrap()),
        )
        .child(switch.build(&p.kit).key("account-ai-features"))
}

/// "Connected AI clients": MCP clients with access to the account, each with "Disconnect".
fn ai_clients(s: &State, p: &Pal) -> El {
    let note = |key: &str, message: &str| text(message).key(key.to_owned()).font(ty::SMALL).color(p.faint).wrap();
    let mut section = col()
        .w_full()
        .gap(8.0)
        .pt(8.0)
        .key("account-ai-clients")
        .child(text("Connected AI clients").font(ty::BODY).color(p.text));
    match &s.ai_connections.view {
        ConnectionsView::NotLoaded | ConnectionsView::Loading => {
            section = section.child(note("account-ai-clients-loading", "Loading…"));
        }
        ConnectionsView::Failed(error) => {
            section = section
                .child(text(error.as_str()).key("account-ai-clients-error").font(ty::SMALL).color(p.accent).wrap())
                .child(
                    Button::new("Try again")
                        .variant(ButtonVariant::Outline)
                        .on_press(|s: &mut State| s.reload_ai_connections())
                        .build(&p.kit)
                        .key("account-ai-clients-retry"),
                );
        }
        ConnectionsView::Loaded(list) if list.is_empty() => {
            section = section.child(note(
                "account-ai-clients-empty",
                "None yet. Add this server's /mcp URL as a connector in Claude or another MCP client.",
            ));
        }
        ConnectionsView::Loaded(list) => {
            for connection in list {
                section = section.child(ai_client_row(s, p, connection));
            }
        }
    }
    section
}

/// A connected client; its "Disconnect" asks first.
fn ai_client_row(s: &State, p: &Pal, connection: &AiConnection) -> El {
    let id = connection.id;
    let confirming = s.ai_connections.confirming.as_ref().is_some_and(|c| c.id == id);
    let detail = if confirming {
        "Disconnect? Its access stops at once; it has to be connected again to use Quest.".to_owned()
    } else {
        connection_detail(connection, s.today(), clock::local_day)
    };
    let mut line = row().w_full().gap(8.0).center_y().child(
        col()
            .grow(1.0)
            .min_w(0.0)
            .gap(2.0)
            .child(text(connection_name(connection)).font(ty::BODY).color(p.text))
            .child(text(detail).font(ty::SMALL).color(if confirming { p.accent } else { p.faint }).wrap()),
    );
    if confirming {
        line = line
            .child(
                Button::new("Keep")
                    .variant(ButtonVariant::Outline)
                    .on_press(|s: &mut State| s.keep_ai_connection())
                    .build(&p.kit)
                    .key(format!("account-ai-keep-{id}")),
            )
            .child(
                Button::new("Disconnect")
                    .variant(ButtonVariant::Contrast)
                    .on_press(|s: &mut State| s.revoke_ai_connection())
                    .build(&p.kit)
                    .key(format!("account-ai-confirm-{id}")),
            );
    } else {
        let asked = connection.clone();
        line = line.child(
            Button::new("Disconnect")
                .variant(ButtonVariant::Outline)
                .on_press(move |s: &mut State| s.ask_revoke_ai_connection(&asked))
                .build(&p.kit)
                .key(format!("account-ai-revoke-{id}")),
        );
    }
    line
}

fn input(key: &str, edit: &TextEdit, placeholder: &str, p: &Pal, bind: fn(&mut State) -> &mut TextEdit) -> El {
    let field = text_input(edit, ty::BODY)
        .key(key.to_owned())
        .label(placeholder)
        .single_line()
        .grow(1.0)
        .color(p.text)
        .placeholder(placeholder, p.faint)
        .on_edit(bind)
        .on_submit(|s: &mut State, _: &mut Tx| s.start_sign_in());
    row().w_full().h(38.0).px(12.0).center_y().radius(8.0).fill(p.bg).border(1.0, p.border_strong).child(field)
}
