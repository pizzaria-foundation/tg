//! The dialog list, described instead of drawn.
//!
//! The same screen as [`crate::chats`], built with `symbian-decl-ui` rather than by placing rects.
//! It exists **beside** the original rather than replacing it, for one reason: a migration whose
//! only evidence is "it looks about right" is a migration that has quietly changed something. Both
//! screens are rendered from the same `Store` and compared pixel for pixel by
//! `examples/chats_parity.rs`; until that comparison is clean, the hand-written one is what ships.
//!
//! # What the declarative version is allowed to be
//!
//! Not a re-imagining. Every colour, every padding, every truncation rule here is the one
//! `chats.rs` already uses, reached through the same `symbian_ui::chrome` calls. The point of the
//! exercise is to find out whether the layer can express a real screen — not whether a nicer screen
//! is possible. Anywhere the two differ, the difference is a finding, and the finding gets read
//! before either side is adjusted.
//!
//! # The row
//!
//! ```text
//!   ┌────────────────────────────────────────────────────┐
//!   │ ( CE )  Carlos Eduardo Nogueira            12:11    │
//!   │         ✓ vou passar aí amanhã               (3)    │
//!   └────────────────────────────────────────────────────┘
//!     avatar   name / preview column, fills       time / badge
//!     fixed    what is left                       measured
//! ```
//!
//! Which is a `Row` of three things: a fixed avatar, a `Column` that fills, and a `Column` sized to
//! its own content. The hand-written version computes that with five rect constructions and two
//! running `right` cursors; the declarative one states it. That is the whole claim being tested.

use symbian_decl_ui::slot::SlotTable;
use symbian_decl_ui::theme::FontRole;
use symbian_decl_ui::widgets::{Avatar, Badge, Column, Ink, Row, Screen, ScrollList, Text, TitleBar};
use symbian_decl_ui::{CrossAlign, MainAlign, Node, Softkeys};
use symbian_ui::gfx::Edges;
use symbian_ui::Theme;

use crate::model::{Chat, Store};

/// What the list can ask the application to do.
///
/// The same three the hand-written screen raises, as values rather than as an enum returned from
/// `handle_key` — the softkeys carry them, so the label and the message are one declaration and
/// cannot drift apart. See `symbian-decl-ui`'s `keys` module for why that matters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// Ask the server for the dialog list again.
    Refresh,
    /// Open the highlighted chat.
    Open,
    /// Leave the application.
    Quit,
    /// The cursor moved to this row.
    Select(usize),
    /// The cursor reached the bottom and there may be more dialogs to fetch.
    LoadMore,
}

/// Build the dialog list for this store.
///
/// `selected` is the application's, not the widget's: which chat is highlighted decides which one
/// `Msg::Open` opens, so it is model state. The scroll *offset* is the slot table's, because it is
/// a function of the viewport height and the model has no business knowing that.
pub fn view(store: &Store, selected: usize, slots: &mut SlotTable) -> Node {
    // "Atualizar" becomes "..." while a request is in flight — the same signal the hand-written
    // screen gives, and the only feedback there is, since there is no push subscription behind
    // this list and no other way to tell that anything is happening.
    let refresh = if store.dialogs_loading { "..." } else { "Atualizar" };
    let subtitle = if store.dialogs_loading { "carregando…" } else { store.status.as_str() };

    // `Screen::content` takes one widget, so the two states are built as two whole screens rather
    // than as one screen with a branch inside it. That reads oddly at first and is the honest
    // shape: an empty list and a full one are different screens, and the alternative — boxing both
    // into a `dyn Widget` — would put the list behind a trait object and lose its own caching.
    let bar = TitleBar::new("Telegram").detail(subtitle);
    if store.chats.is_empty() {
        // A placeholder rather than an empty list: "Nenhuma conversa" is an answer, and a blank
        // panel is indistinguishable from a screen that failed to draw.
        return Node::leaf(
            Screen::new()
                .title_bar(bar)
                .content(Text::new("Nenhuma conversa").ink(Ink::Dim).align(symbian_ui::Align::Center))
                .on_options(refresh, Msg::Refresh)
                .on_action("Abrir", Msg::Open)
                .on_back("Sair", Msg::Quit),
        );
    }

    let chats = store.chats.clone();
    Node::leaf(
        Screen::new()
            // `TitleBar::detail` is the subtitle slot the hand-written screen already uses for the
            // loading state; the same one, reached declaratively.
            .title_bar(bar)
            .content(
                ScrollList::new(slots, store.chats.len(), row_height())
                    .selected(selected)
                    .scrollbar(true)
                    .row(move |i, is_selected| chat_row(&chats[i], is_selected)),
            )
            .on_options(refresh, Msg::Refresh)
            .on_action("Abrir", Msg::Open)
            .on_back("Sair", Msg::Quit),
    )
}

/// One dialog.
///
/// Every colour decision is deferred to the theme role the hand-written row uses, so the two cannot
/// drift apart when the palette changes: name and time go to `text`/`dim` normally and both to
/// `selection_text` under the highlight, which is what makes a selected row read as one block.
fn chat_row(chat: &Chat, selected: bool) -> Node {
    // Under the highlight every line goes to the selection colour, which is what makes a selected
    // row read as one block rather than as a band with ordinary text on it.
    let name_ink = if selected { Ink::Selection } else { Ink::Text };
    let sub_ink = if selected { Ink::Selection } else { Ink::Dim };

    // Two stacked lines, not two side-by-side columns.
    //
    // The obvious reading of the hand-written row is "avatar | text column | time-and-badge
    // column", and it is wrong in a way only the pixels show: the preview is allowed to run *under*
    // the timestamp, because nothing in the original constrains it to a column — each piece is
    // placed against `r.x1` on its own. Modelled as two columns, the preview stops short and
    // truncates a character early, which is the last difference the comparison found.
    //
    // Modelled as two rows it is both faithful and, arguably, what the design always was: a line
    // with the name and the time on it, and a line with the preview and the unread count.
    let time = Text::new(&chat.time).font(FontRole::Small).ink(sub_ink);
    let top_line = Row::new()
        .child(Text::new(&chat.name).font(FontRole::Strong).ink(name_ink).flex(1))
        // The timestamp sits a pixel below the name's cap line in the original — `r.y0 + 4` against
        // the name's `r.y0 + 3` — and a pixel is visible when two lines of different sizes share a
        // baseline. Its own padding says so rather than the row being nudged for it.
        .group(Column::new().padding(Edges::new(0, 1, 0, 0)).child(time));

    let preview_text = Text::new(chat.preview()).font(FontRole::Small).ink(sub_ink).flex(1);
    // `align-items: flex-end`: the badge is two pixels taller than a line of small text, and the
    // original anchors both to `r.y1 - 4`. Aligned to the top instead, the badge hangs two pixels
    // below the preview's baseline.
    //
    // `overflow: visible`, because the pill genuinely does not fit. This line is as tall as the
    // small text in it; the pill is two pixels taller and the original reaches into the name's line
    // box by exactly that. Both halves of the escape are needed and they are different mechanisms:
    // `Badge` declares `Widget::overflow_visible` so the engine does not clip it to its own rect,
    // and this row declares `Group::overflow_visible` so the line it sits in does not clip it
    // either. Either one alone leaves a flat lid on the circle.
    let mut bottom_line = Row::new().align(CrossAlign::End).overflow_visible();
    if chat.last_outgoing {
        // The tick is its own colour, so it is its own widget: concatenating it into the preview
        // string made it take the preview's ink.
        let tick_ink = if selected { Ink::Selection } else { Ink::Accent };
        bottom_line = bottom_line.child(Text::new("\u{2713} ").font(FontRole::Small).ink(tick_ink));
    }
    bottom_line = bottom_line.child(preview_text);
    if let Some(badge) = Badge::count(chat.unread, selected) {
        bottom_line = bottom_line.child(badge);
    }

    // `padding: 3px 0 4px` — the original anchors the name at `y0 + 3` and the preview's baseline
    // at `y1 - 4`, two insets that are not the same number and never were. Stated as the column's
    // padding rather than as two anchors, which is where CSS would put it.
    //
    // No `overflow_visible` here, and it is worth saying why not: the overlap is *within* this
    // column — the pill reaches up into the name's line, not out of the column — so this clip never
    // touches it. Declaring it anyway would read as necessary and be untrue. The list still clips
    // at its own edge, which is the boundary that matters: a row may overlap its own lines, and may
    // not paint on the title bar.
    let text_column = Column::new()
        .justify(MainAlign::SpaceBetween)
        .padding(Edges::new(0, 3, 0, 4))
        .group(top_line)
        .group(bottom_line)
        .fill(1);

    // The separator, as a border rather than a child — see `Group::border_bottom`. Skipped under
    // the highlight so a selected row reads as one solid block, which is what the hand-written row
    // does and the reason it checks `!selected` before drawing its `hline`.
    //
    // `align-items: stretch`, not `center`: a column with one child is as tall as that child, and
    // centring it puts a lone timestamp in the middle of the row instead of at its top. The avatar
    // is unaffected — `chrome::avatar` centres a circle of `min(w, h)` in whatever rect it gets.
    let mut row = Row::new().padding(Edges::xy(PAD, 0)).gap(PAD).align(CrossAlign::Stretch);
    if !selected {
        row = row.border_bottom(Ink::Divider, PAD);
    }
    Node::Group(
        row.child(Avatar::new(chat.initials(), chat.color_seed()).size(row_height() - 8))
            .group(text_column),
    )
}

/// The row's inset, which is `theme.metrics.pad`.
///
/// Hardcoded to 4 at first, against a real value of 5, and every line of text in the list came out
/// a pixel to the left — while the avatars, sized from the row height instead, landed exactly.
/// That is the whole hazard of a declarative translation: a number copied by eye agrees with the
/// original everywhere it is not used. The comparison found it; nothing else would have.
const PAD: i32 = 5;

/// Row height. The hand-written list uses `theme.metrics.row_h`; this is the same number, and the
/// duplication is why `examples/chats_parity.rs` asserts on it rather than trusting it.
fn row_height() -> i32 {
    38
}

/// The keys this screen handles beyond the softkey convention.
///
/// Down at the bottom of the list asks for another page. It is checked *before* the list's own
/// navigation, exactly as `chats.rs` does — the comment there is worth repeating, because it is the
/// kind of thing that only breaks once: dispatched afterwards, the list would have already clamped
/// the cursor at the last row and the key would look handled.
pub fn extra_key(
    ev: symbian_ui::KeyEvent,
    store: &Store,
    selected: usize,
    viewport_h: i32,
    theme: &Theme<'_>,
) -> Option<Msg> {
    let _ = theme;
    if ev.key != symbian_ui::Key::Down || store.chats.is_empty() {
        return None;
    }
    let count = store.chats.len();
    let content_h = count as i32 * row_height();
    if selected == count - 1 && content_h > viewport_h {
        return Some(Msg::LoadMore);
    }
    None
}

/// The softkey labels, for a caller that wants to draw the bar without building the whole screen.
pub fn softkeys(store: &Store) -> Softkeys<Msg> {
    let refresh = if store.dialogs_loading { "..." } else { "Atualizar" };
    Softkeys::new()
        .options(refresh, Msg::Refresh)
        .action("Abrir", Msg::Open)
        .back("Sair", Msg::Quit)
}

