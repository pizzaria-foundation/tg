//! The three login screens, described instead of drawn.
//!
//! The same screens as [`crate::login`]'s `draw`, built with `symbian-decl-ui`, and compared with it
//! pixel for pixel by `examples/login_parity.rs`. The hand-written one is the reference: it is what
//! ships, and where the two differ the difference is a finding.
//!
//! # The shape, which is three layers and not one column
//!
//! ```text
//!   ┌─────────────────────────────────────────┐  title bar
//!   ├─────────────────────────────────────────┤
//!   │                                         │
//!   │            Número de telefone           │  ┐ centred in the *whole* band
//!   │        ┌───────────────────────┐        │  │ title, gap, field, error
//!   │        │ + 11 999999999        │        │  ┘
//!   │                                         │
//!   │            Digite o código…             │  ← pinned: pad + softkey_h from the bottom
//!   │              conectando…                │  ← pinned: pad from the bottom
//!   ├─────────────────────────────────────────┤
//!   │        Voltar      Entrar               │  softkey bar
//!   └─────────────────────────────────────────┘
//! ```
//!
//! The block is centred in the band *including* the space the two bottom lines sit in — the
//! hand-written screen centres against `frame.content` and then writes the status over it. As a
//! column those three would compete for the axis and the block would sit half a line high; as
//! [`Stack`] layers it is what the original does. That is the whole reason `Stack` exists.
//!
//! # The field is one drawing, in the toolkit
//!
//! `chrome::text_field` draws the box, the `+`, the mask, the selection and the caret, and both this
//! screen and `login.rs` call it. It used to be written out in `login.rs` while the declarative
//! widget drew a different field — a stroked rectangle, a caret elsewhere — and two drawings of one
//! control can never be compared. See that function's header.
//!
//! # What this module deliberately does not have
//!
//! The login *machine*. `login.rs` owns the protocol and the transitions; this owns a description of
//! what is on screen. The state below is that description and nothing more, which is what lets the
//! parity harness build a screen that the machine could not be talked into producing.

use alloc::rc::Rc;
use alloc::string::String;
use core::cell::RefCell;

use symbian_decl_ui::slot::SlotTable;
use symbian_decl_ui::theme::FontRole;
use symbian_decl_ui::widgets::{Column, Ink, Row, Screen, Spacer, Stack, Text, TextField, TitleBar};
use symbian_decl_ui::{CrossAlign, Key, KeyEvent, MainAlign, Node, Softkeys};
use symbian_ui::gfx::Edges;
use symbian_ui::{edit, Align, Metrics};

/// What the login screens can ask the application to do.
///
/// Deliberately not [`crate::login::LoginAction`]: that carries the *contents* of the field, because
/// the hand-written screen read them while it had the field in hand. Here the field's buffer belongs
/// to the application, so the message says what was pressed and `update` reads the text — which is
/// also what keeps a password out of a message that could be logged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// The middle key: send the code, submit the code, submit the password — whichever screen this
    /// is. One variant rather than three, because the screen already says which.
    Submit,
    /// Back to the phone number: the code screen's Voltar, and the waiting screen's "Cancelar".
    BackToPhone,
    /// Show or hide the password.
    ToggleMask,
    /// Leave the application.
    Quit,
}

/// Which login screen, and what it needs beyond the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Which {
    /// The phone number, with the fixed `+` drawn in front of the field.
    Phone {
        /// Set when this build has no `api_id`, which is said here instead of after a round trip.
        credentials_missing: bool,
    },
    /// The code the server sent. `length` is how many digits it said to expect.
    Code { length: Option<i32> },
    /// The two-factor password, with the server's hint under it.
    Password { hint: String, masked: bool },
    /// Waiting for the network or the worker; the message is the only thing on screen.
    Waiting(String),
}

/// Everything the login screens draw, without the machine that drives them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub which: Which,
    /// The error line under the field, if there is one.
    pub error: Option<String>,
    /// The connection status, along the bottom. Empty means hidden.
    pub status: String,
    /// Whether the connection is ready. An unready one hides the middle key rather than offering
    /// something that cannot work.
    pub connected: bool,
}

impl State {
    /// Describe what the login machine currently has on screen.
    ///
    /// The one place the imperative state becomes a description, so the parity harness can build a
    /// state the machine could not be talked into producing while the application always builds one
    /// that matches it exactly.
    pub(crate) fn of(login: &crate::login::Login) -> Self {
        use crate::login::Screen as S;
        let which = match &login.screen {
            S::Phone { .. } => Which::Phone { credentials_missing: login.credentials_missing() },
            S::Code { length, .. } => Which::Code { length: *length },
            S::Password { field, hint, .. } => {
                Which::Password { hint: hint.clone(), masked: field.borrow().is_masked() }
            }
            S::Waiting(msg) => Which::Waiting(String::from(*msg)),
        };
        let error = match &login.screen {
            S::Phone { error, .. } | S::Code { error, .. } | S::Password { error, .. } => {
                error.clone()
            }
            S::Waiting(_) => None,
        };
        Self {
            which,
            error,
            // The waiting screen shows its message in the middle of the band and does not write a
            // status along the bottom as well — the hand-written screen does not call `draw_status`
            // there, and the message is the status.
            status: match &login.screen {
                S::Waiting(_) => String::new(),
                _ => String::from(login.status),
            },
            connected: login.connected,
        }
    }
}

/// The theme metrics a view is allowed to know — see `chats_decl`'s note on why this is
/// `Metrics::default()` and not `theme.metrics`.
fn metrics() -> Metrics {
    Metrics::default()
}

/// Build the screen for this state, over a buffer the application owns.
///
/// The buffer is a parameter and not a slot, and that is the login screen's one structural
/// difference from the dialog list: the middle softkey is answered by the *application*, so `update`
/// has to be able to read what was typed — and a slot cannot be reached from `update`. See
/// [`TextField::with_buffer`].
pub fn view(
    state: &State,
    field: &Rc<RefCell<edit::TextField>>,
    slots: &mut SlotTable,
) -> Node {
    let m = metrics();
    let detail = match &state.which {
        Which::Phone { .. } => crate::strings::title_sign_in(),
        Which::Code { .. } => crate::strings::code(),
        Which::Password { .. } => crate::strings::password(),
        Which::Waiting(msg) => msg.as_str(),
    };
    let bar = TitleBar::new("Telegram").detail(detail);

    // Waiting is a different screen, not this one with the field hidden: a placeholder in the middle
    // of the band, and the only key is the way out.
    if let Which::Waiting(msg) = &state.which {
        return Node::leaf(
            Screen::new()
                .title_bar(bar)
                .content(Text::new(msg.as_str()).ink(Ink::Dim).align(Align::Center))
                .softkeys(softkeys(state))
                .keep_softkey_band(),
        );
    }

    let (title, prefix, placeholder) = match &state.which {
        Which::Phone { .. } => (crate::strings::phone_number(), Some("+"), "11 999999999"),
        Which::Code { .. } => (crate::strings::code_field(), None, crate::strings::code()),
        Which::Password { .. } => (crate::strings::two_factor_password(), None, crate::strings::password()),
        Which::Waiting(_) => unreachable!("handled above"),
    };

    // A build with no api_id reaches Telegram and is told API_ID_INVALID. Said here instead, before
    // a number is typed and a round trip is spent, and said *in place of* the error line rather than
    // beside it — there is no error yet and inventing one would be a lie about what happened.
    let line = match &state.which {
        Which::Phone { credentials_missing: true } => {
            Some(String::from(crate::strings::no_api_id()))
        }
        _ => state.error.clone(),
    };

    let masked = matches!(state.which, Which::Password { masked: true, .. });
    let mut field_widget = TextField::with_buffer(field.clone()).focused(true).masked(masked);
    if let Some(pre) = prefix {
        field_widget = field_widget.prefix(pre);
    }
    field_widget = field_widget.placeholder(placeholder);

    // The centred block. `justify(Center)` is the `area.y0 + (height - total) / 2` the hand-written
    // screen computes; `Stretch` gives every child the full padded width, so a centred label is
    // centred in the band and not in its own ink.
    let mut block = Column::new()
        .padding(Edges::xy(m.pad, 0))
        .fill(1)
        .stretch_width()
        .justify(MainAlign::Center)
        .align(CrossAlign::Stretch)
        .child(Text::new(title).font(FontRole::Title).align(Align::Center))
        .child(Spacer::new().height(8))
        // The field is narrower than the band, and what sits beside it must not move it: the eye
        // hangs off the right edge of the *field*, and a row that included it in its own width would
        // shift the field left by half the eye. So the row is placed from the left by the same
        // arithmetic the hand-written screen uses — `(band - field) / 2` — and whatever follows the
        // field simply follows it.
        .group({
            let mut row = Row::new()
                .align(CrossAlign::Center)
                .child(Spacer::new().width(field_indent()))
                .group(
                    Column::new()
                        .width(field_width())
                        .align(CrossAlign::Stretch)
                        .child(field_widget),
                );
            if matches!(state.which, Which::Password { .. }) {
                // Six pixels after the field, vertically centred on it — `draw_eye`'s own numbers,
                // which is why this is one widget rather than an attempt to describe two arcs.
                row = row
                    .child(Spacer::new().width(6))
                    .child(Eye { open: !masked });
            }
            row
        });
    if let Some(err) = line {
        // Two pixels above the text and two below: the original's error band is
        // `small.line_height() + 4` tall and its text sits at the top of it.
        block = block
            .child(Spacer::new().height(2))
            .child(Text::new(err).font(FontRole::Small).ink(Ink::Unread).align(Align::Center))
            .child(Spacer::new().height(2));
    }

    let mut stack = Stack::new(slots).group(block);

    // The hint, one softkey-height above the status line. Its own layer with its own bottom padding
    // rather than a gap between the two, because the gap is `softkey_h - small.line_height()` and a
    // view has no font to measure with.
    if let Some(hint) = hint(&state.which) {
        stack = stack.group(
            Column::new()
                .fill(1)
                .stretch_width()
                .justify(MainAlign::End)
                .align(CrossAlign::Stretch)
                .padding(Edges::new(m.pad, 0, m.pad, m.pad + m.softkey_h))
                .child(Text::new(hint).font(FontRole::Small).ink(Ink::Dim).align(Align::Center)),
        );
    }

    if !state.status.is_empty() {
        // The status box starts at the band's left edge and is `width - 2 * pad` wide — not centred,
        // which is what the original does and is visible on a long status. Reproduced as padding on
        // the right only.
        stack = stack.group(
            Column::new()
                .fill(1)
                .stretch_width()
                .justify(MainAlign::End)
                .align(CrossAlign::Stretch)
                .padding(Edges::new(0, 0, m.pad * 2, m.pad))
                .child(
                    Text::new(state.status.as_str())
                        .font(FontRole::Small)
                        .ink(Ink::Accent)
                        .align(Align::Center),
                ),
        );
    }

    // The band stays even when there is nothing to offer — an unready connection hides the middle
    // key, and the hand-written screen still draws the bar. Dropping it moves everything centred in
    // the content band by seventeen pixels.
    Node::leaf(
        Screen::new()
            .title_bar(bar)
            .content(stack)
            .softkeys(softkeys(state))
            .keep_softkey_band(),
    )
}

/// The field's width: the original's `area.width() / 2 + 40`.
///
/// The one number in this module that needs the *screen* and not a metric, because
/// `Frame::split` divides vertically only — the content band is as wide as the display. A view is
/// built without a rect, so this is the assumption, and it is the same one `Metrics` already makes
/// ("chosen against 320x240"). The parity comparison renders at `E72_SCREEN` and would fail on
/// every scene if the display were ever a different width, which is the guard.
fn field_width() -> i32 {
    symbian_ui::gfx::E72_SCREEN.w / 2 + 40
}

/// How far in from the band's left edge the field starts: `(band - field) / 2`, the hand-written
/// screen's own centring, written as an inset because something may sit beside the field.
fn field_indent() -> i32 {
    let m = metrics();
    (symbian_ui::gfx::E72_SCREEN.w - m.pad * 2 - field_width()) / 2
}

/// The eye beside the password field, saying whether the text is visible.
///
/// A leaf, because it is drawing rather than layout — two arcs, a pupil, and a slash when hidden.
/// `docs/decl-ui.md` says to leave that kind of thing alone, and this is the smallest possible
/// version of leaving it alone: the pixels are `login::draw_eye`'s, reached through the toolkit's own
/// escape hatch rather than described as a tree of rectangles.
struct Eye {
    open: bool,
}

impl symbian_decl_ui::Widget for Eye {
    fn content_hash(&self) -> symbian_decl_ui::WidgetHash {
        symbian_decl_ui::widget::hash_i32(0, self.open as i32)
    }

    fn measure(
        &self,
        c: symbian_decl_ui::Constraints,
        _t: &symbian_ui::Theme<'_>,
    ) -> symbian_ui::Size {
        // The size `draw_eye` draws: fourteen by nine, and not a pixel of it is negotiable, because
        // the shape is built from that ratio.
        c.constrain(symbian_ui::Size::new(crate::login::EYE_W, crate::login::EYE_H))
    }

    fn draw(
        &self,
        c: &mut symbian_ui::Canvas<'_>,
        rect: symbian_ui::Rect,
        theme: &symbian_ui::Theme<'_>,
    ) {
        crate::login::draw_eye_at(c, rect, theme, self.open);
    }
}

/// The line under the field: how many digits to expect, or the server's password hint.
fn hint(which: &Which) -> Option<String> {
    match which {
        Which::Code { length: Some(n) } => {
            let mut s = String::from(crate::strings::enter_the());
            s.push_str(&crate::login::itoa(*n as u32));
            s.push_str(crate::strings::digits_suffix());
            Some(s)
        }
        Which::Code { length: None } => Some(String::from(crate::strings::enter_sms_code())),
        Which::Password { hint, .. } if !hint.is_empty() => Some(hint.clone()),
        _ => None,
    }
}

/// The softkey bar for this state.
///
/// One declaration, drawn by [`view`] and dispatched by [`on_key`]. The middle key disappears rather
/// than greying out when the connection is not ready — there is no disabled state in this toolkit,
/// and a label that did nothing would be the defect `symbian-decl-ui`'s `keys` module exists to make
/// impossible.
pub fn softkeys(state: &State) -> Softkeys<Msg> {
    let bar = Softkeys::new();
    match &state.which {
        Which::Waiting(_) => bar.back(symbian_ui::strings::cancel(), Msg::BackToPhone),
        Which::Phone { credentials_missing } => {
            if *credentials_missing || !state.connected {
                bar
            } else {
                bar.action(crate::strings::next(), Msg::Submit)
            }
        }
        Which::Code { .. } => {
            let bar = bar.options(symbian_ui::strings::back(), Msg::BackToPhone);
            if state.connected {
                bar.action(crate::strings::sign_in(), Msg::Submit)
            } else {
                bar
            }
        }
        Which::Password { masked, .. } => {
            // The label says what pressing it will do, not what the field is doing now — a softkey
            // is a verb.
            let bar = bar.options(if *masked { crate::strings::show() } else { crate::strings::hide() }, Msg::ToggleMask);
            if state.connected {
                bar.action(crate::strings::sign_in(), Msg::Submit)
            } else {
                bar
            }
        }
    }
}

/// What a key means on these screens, before the field sees it.
///
/// # Two keys the bar cannot carry
///
/// `Key::Call` submits, as it does everywhere on this phone and as `handle_screen_key` always did.
///
/// And the phone screen answers `Softkey::Right` with *no label on the bar*. That is the very defect
/// `symbian-decl-ui`'s `keys` module was written against — a key that does something it never says —
/// and it is preserved here rather than fixed, because fixing it means drawing a third label and the
/// comparison against the shipping screen would fail on the pixels. It is bound here, outside the
/// bar, so the binding is at least visible; `Key::End` does the same thing from every screen.
///
/// Navigation and typing are absent: they belong to the field, which the tree hands them to.
pub fn on_key(state: &State, ev: KeyEvent) -> Option<Msg> {
    let has_field = !matches!(state.which, Which::Waiting(_));
    match ev.key {
        // Submit, whatever the bar says. The hand-written screen answers `Select`, `Enter`,
        // `Softkey(Middle)` and `Call` on every screen with a field, *including* when the middle
        // label is hidden because the connection is not ready — and then reports what came back, so
        // pressing it while the link is down produces an error rather than silence.
        //
        // That is a key doing something the bar does not advertise, which is exactly what
        // `symbian-decl-ui`'s `keys` module exists to prevent, and it is preserved here rather than
        // tidied: with the label hidden the honest fix is to *show* it and let the submit say "sem
        // conexao", and that changes the pixels this screen is being compared against. Worth doing
        // deliberately, in its own change, with the comparison updated on purpose.
        Key::Call | Key::Select | Key::Enter | Key::Softkey(symbian_ui::Softkey::Middle)
            if has_field =>
        {
            Some(Msg::Submit)
        }
        // The phone screen answers the right softkey with nothing on the bar — the same defect, in
        // the same original. Bound here so the binding is at least visible; `Key::End` does the same
        // thing from every screen.
        Key::Softkey(symbian_ui::Softkey::Right)
            if matches!(state.which, Which::Phone { .. }) =>
        {
            Some(Msg::Quit)
        }
        _ => softkeys(state).dispatch(ev),
    }
}
