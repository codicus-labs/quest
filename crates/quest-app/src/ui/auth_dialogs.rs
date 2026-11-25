//! Dialogs of the account flows: importing guest data after an account's first sign-in, and the
//! privacy notice before AI features turn on.

use rune::{El, col, row, spacer, text};
use rune_kit::{Button, ButtonVariant, Dialog};

use crate::state::State;
use crate::theme::{Pal, ty};

/// Exactly what AI capture sends, as agreed for the opt-in.
pub const AI_PRIVACY: &str = "When you capture with AI, Quest sends this through your Quest server to the AI \
provider it is configured with: the text you type or dictate; the titles and shortened descriptions of your \
active quests; the titles of your tasks from 7 days ago to 14 days ahead, plus the backlog; the current time, \
your time zone and your language. It sends no task descriptions, no time entries and not your email address. \
Nothing is created until you confirm.";

pub fn guest_import_layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let k = &p.kit;
    let mut body = col().key("guest-import").w_full().gap(16.0).px(20.0).pb(20.0).child(
        text(
            "This Mac has quests and tasks you made without signing in. Import them into this account? \
                 If not, they stay in the guest data, which shows when you are signed out.",
        )
        .font(ty::BODY)
        .color(p.text)
        .wrap(),
    );
    if let Some(error) = &s.auth.import_error {
        body = body.child(
            text(format!("{error} The guest data is unchanged."))
                .key("guest-import-error")
                .role("alert")
                .font(ty::SMALL)
                .color(p.accent)
                .wrap(),
        );
    }
    let import_label = if s.auth.import_error.is_some() { "Try again" } else { "Yes, import" };
    let body = body.child(
        row()
            .w_full()
            .gap(10.0)
            .child(spacer())
            .child(
                Button::new("No")
                    .variant(ButtonVariant::Ghost)
                    .on_press(|s: &mut State| s.keep_guest_data())
                    .build(k)
                    .key("guest-import-no"),
            )
            .child(
                Button::new(import_label)
                    .variant(ButtonVariant::Contrast)
                    .on_press(|s: &mut State| s.import_guest())
                    .build(k)
                    .key("guest-import-yes"),
            ),
    );
    Dialog::new("guest-import-dialog", "Import guest data into this account?")
        .open(true)
        .fit()
        .width(460.0)
        .on_close(|s: &mut State| s.keep_guest_data())
        .child(body)
        .build(k, layer);
}

pub fn ai_notice_layer(p: &Pal, layer: &mut Vec<El>) {
    let k = &p.kit;
    let body = col()
        .key("ai-notice")
        .w_full()
        .gap(16.0)
        .px(20.0)
        .pb(20.0)
        .child(text(AI_PRIVACY).key("ai-notice-text").font(ty::BODY).color(p.text).wrap())
        .child(text("You can turn AI features off again in Account & sync (⌘,).").font(ty::SMALL).color(p.faint).wrap())
        .child(
            row()
                .w_full()
                .gap(10.0)
                .child(spacer())
                .child(
                    Button::new("Cancel")
                        .variant(ButtonVariant::Ghost)
                        .on_press(|s: &mut State| s.decline_ai_notice())
                        .build(k)
                        .key("ai-notice-cancel"),
                )
                .child(
                    Button::new("Turn on")
                        .variant(ButtonVariant::Contrast)
                        .on_press(|s: &mut State| s.accept_ai_notice())
                        .build(k)
                        .key("ai-notice-accept"),
                ),
        );
    Dialog::new("ai-notice-dialog", "Turn on AI features?")
        .open(true)
        .fit()
        .width(520.0)
        .on_close(|s: &mut State| s.decline_ai_notice())
        .child(body)
        .build(k, layer);
}
