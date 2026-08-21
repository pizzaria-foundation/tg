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

use alloc::rc::Rc;
use alloc::string::String;

use symbian_decl_ui::outbox::Outbox;
use symbian_decl_ui::slot::SlotTable;
use symbian_decl_ui::theme::FontRole;
use symbian_decl_ui::widgets::{
    scroll_list::Edge, Avatar, Badge, Column, Ink, Row, Screen, ScrollList, Text, TitleBar,
};
use symbian_decl_ui::{CrossAlign, Key, KeyEvent, MainAlign, Node, Softkeys};
use symbian_ui::gfx::Edges;
use symbian_ui::Metrics;

use crate::model::{Chat, Store};

/// What the list can ask the application to do.
///
/// The same three the hand-written screen raises, as values rather than as an enum returned from
/// `handle_key` — the softkeys carry them, so the label and the message are one declaration and
/// cannot drift apart. See `symbian-decl-ui`'s `keys` module for why that matters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// Ask the server for the dialog list again. Reached through the Options menu now, not from a
    /// softkey of its own — see [`crate::menu_decl`].
    Refresh,
    /// Open the Options menu.
    Options,
    /// Open the highlighted chat.
    Open,
    /// Leave the application.
    Quit,
    /// The cursor moved to this row.
    Select(usize),
    /// The cursor reached the bottom and there may be more dialogs to fetch.
    LoadMore,
}

/// One row's worth of a chat: exactly what the list draws, and nothing else.
///
/// # Why this type exists
///
/// The first version of this screen captured `store.chats.clone()` in its row closure, once per
/// rebuild. A [`Chat`] carries `Vec<Message>`, and a message can carry a complete inline JPEG in
/// `Media::preview` — so against a real account with two hundred dialogs that clone is megabytes,
/// copied on every keypress that changes the model, on a phone with a 4 MB heap and a non-compacting
/// allocator. Nothing had noticed because the mock store has seven chats and no previews.
///
/// A row needs seven small values. This is them, with the two derived ones — the initials and the
/// avatar tint — computed once at projection rather than per frame per visible row.
///
/// The projection is `Rc<[ChatRow]>` so that a view rebuilt for an unrelated reason costs a
/// reference count and not a copy: the app holds the slice and hands the same one to
/// [`view_rows`] until the dialog list itself changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatRow {
    /// One or two letters, from the first two words of the name.
    pub initials: String,
    pub name: String,
    pub time: String,
    /// The one line under the name — the last message, or a label like `[foto]` for media. Owned,
    /// and short: `Chat::preview` returns a literal for every media kind, so this never carries
    /// image bytes.
    pub preview: String,
    pub unread: u32,
    /// Whether the last message in the preview is ours, for the leading tick.
    pub last_outgoing: bool,
    /// Stable per-name avatar tint.
    pub color_seed: u32,
}

impl ChatRow {
    pub fn of(chat: &Chat) -> Self {
        Self {
            initials: chat.initials(),
            name: chat.name.clone(),
            time: chat.time.clone(),
            preview: chat.preview().into(),
            unread: chat.unread,
            last_outgoing: chat.last_outgoing,
            color_seed: chat.color_seed(),
        }
    }
}

/// Project the store's dialogs into rows.
///
/// Call it when the dialog list changes, not when the view is rebuilt — the point of
/// [`ChatRow`] is that a keypress which moves the cursor can re-use the same slice.
pub fn rows(store: &Store) -> Rc<[ChatRow]> {
    store.chats.iter().map(ChatRow::of).collect()
}

/// Build the dialog list.
///
/// Takes the rows already projected rather than the store, so the projection is visible at the call
/// site: it is the one expensive thing this screen does per rebuild, and the caller is the only party
/// in a position to keep the `Rc` and hand the same one back next time.
///
/// `selected` is the application's, not the widget's: which chat is highlighted decides which one
/// `Msg::Open` opens, so it is model state. The scroll *offset* is the slot table's, because it is a
/// function of the viewport height and the model has no business knowing that. The cursor is *moved*
/// by the list — see [`ScrollList::on_move`] — for the same reason: `Left` and `Right` page by a
/// screenful, and how many rows fit is a layout fact.
///
/// `out` is where the list reports both of those: where the cursor went, and that Down was pressed
/// with nowhere left to go, which is this screen's request for another page.
pub fn view(
    rows: Rc<[ChatRow]>,
    loading: bool,
    status: &str,
    selected: usize,
    out: &Outbox<Msg>,
    slots: &mut SlotTable,
) -> Node {
    // The subtitle carries the loading state, the same signal the hand-written screen gives: there
    // is no push subscription behind this list and no other way to tell that anything is happening.
    let subtitle = if loading { "carregando…" } else { status };

    // `Screen::content` takes one widget, so the two states are built as two whole screens rather
    // than as one screen with a branch inside it. That reads oddly at first and is the honest
    // shape: an empty list and a full one are different screens, and the alternative — boxing both
    // into a `dyn Widget` — would put the list behind a trait object and lose its own caching.
    let bar = TitleBar::new("Telegram").detail(subtitle);
    if rows.is_empty() {
        // A placeholder rather than an empty list: "Nenhuma conversa" is an answer, and a blank
        // panel is indistinguishable from a screen that failed to draw.
        return Node::leaf(
            Screen::new()
                .title_bar(bar)
                .content(Text::new("Nenhuma conversa").ink(Ink::Dim).align(symbian_ui::Align::Center))
                .softkeys(softkeys(loading)),
        );
    }

    let count = rows.len();
    let (moved, edged) = (out.clone(), out.clone());
    Node::leaf(
        Screen::new()
            // `TitleBar::detail` is the subtitle slot the hand-written screen already uses for the
            // loading state; the same one, reached declaratively.
            .title_bar(bar)
            .content(
                ScrollList::new(slots, count, row_height())
                    .selected(selected)
                    .scrollbar(true)
                    .on_move(move |i| moved.push(Msg::Select(i)))
                    // Down on the last row is how this screen has always asked for the next page —
                    // `chats.rs` checks it before the list's own navigation, and the check there is
                    // "last row *and* scrolled to the bottom". Those are the same condition: the
                    // selection's offset is derived by `ensure_visible`, which parks the last row
                    // against the bottom edge whether the content overflows or not. The equivalence
                    // is asserted in this module's tests rather than left as an argument.
                    .on_edge(move |edge| {
                        if edge == Edge::Bottom {
                            edged.push(Msg::LoadMore);
                        }
                    })
                    .row(move |i, is_selected| chat_row(&rows[i], is_selected)),
            )
            .softkeys(softkeys(loading)),
    )
}

/// One dialog.
///
/// Every colour decision is deferred to the theme role the hand-written row uses, so the two cannot
/// drift apart when the palette changes: name and time go to `text`/`dim` normally and both to
/// `selection_text` under the highlight, which is what makes a selected row read as one block.
fn chat_row(chat: &ChatRow, selected: bool) -> Node {
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

    let preview_text = Text::new(&chat.preview).font(FontRole::Small).ink(sub_ink).flex(1);
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
    let mut row = Row::new().padding(Edges::xy(pad(), 0)).gap(pad()).align(CrossAlign::Stretch);
    if !selected {
        row = row.border_bottom(Ink::Divider, pad());
    }
    Node::Group(
        row.child(Avatar::new(&chat.initials, chat.color_seed).size(row_height() - 8))
            .group(text_column),
    )
}

/// The row's inset, and the row height.
///
/// # Two numbers that were typed by hand, and what that cost
///
/// `PAD` was `4` in the first version of this file, against a real value of `5`, and every line of
/// text in the list came out a pixel to the left — while the avatars, sized from the row height
/// instead, landed exactly. A number copied by eye agrees with the original everywhere it is not
/// used. The comparison found it; nothing else would have.
///
/// So they are not typed any more. They come from [`Metrics`], which is the *same struct every
/// [`Theme`](symbian_ui::Theme) is constructed with* — `Theme::new` fills `metrics` with
/// `Metrics::default()` and nothing in the SDK or in either application has ever written to it. So
/// `Metrics::default().pad` and `theme.metrics.pad` are one value reached two ways.
///
/// # Why not simply read `theme.metrics`
///
/// Because a declarative view is built without a theme, on purpose: colours are named by role
/// ([`Ink`]) and fonts by role ([`FontRole`]), and both are resolved at draw time when a theme is in
/// hand. A row *height* cannot wait that long — it is an argument to `ScrollList::new`, which is
/// called while the tree is being described.
///
/// That leaves this module standing on an assumption: that no theme carries its own metrics.
/// [`the_metrics_here_are_the_metrics_the_theme_uses`] is the guard, and the day someone gives a
/// large-font theme a taller row, it fails here rather than in a screenshot.
fn metrics() -> Metrics {
    Metrics::default()
}

/// The row's inset — `theme.metrics.pad`. Shared with `menu_decl`, whose rows sit in front of this
/// list and would read as a different application if they were inset differently.
pub(crate) fn pad() -> i32 {
    metrics().pad
}

/// Row height — `theme.metrics.row_h`, the same number `chats.rs` passes to [`Uniform`].
pub fn row_height() -> i32 {
    metrics().row_h
}

/// The softkey bar this screen offers.
///
/// One declaration, drawn by [`view`] and dispatched by [`on_key`] — which is the whole point of
/// [`Softkeys`]. "Abrir" is offered even with nothing to open, because the hand-written screen offers
/// it and the hand-written screen is what ships; pressing it on an empty list does nothing, exactly
/// as `ChatList::activate` does nothing. A bar that dropped the label would be a nicer screen and a
/// failed comparison.
pub fn softkeys(loading: bool) -> Softkeys<Msg> {
    // "Opções" rather than "Atualizar": one slot cannot be spent on one verb once there is a second
    // thing to offer, and refreshing is the menu's first entry — see `crate::menu_decl`.
    //
    // "..." while a request is in flight stays, because it is still the only feedback this screen
    // has that anything is happening. The label under it is the menu's, not the refresh's.
    let left = if loading { "..." } else { "Opções" };
    Softkeys::new().options(left, Msg::Options).action("Abrir", Msg::Open).back("Sair", Msg::Quit)
}

/// What a key means on this screen, before any widget sees it.
///
/// Everything the softkey convention covers, plus the green key. `Key::Call` opens the highlighted
/// chat on this hardware — it is what a thumb reaches for after a name is highlighted, and
/// `ChatList::activate` has always honoured it — and [`Softkeys::dispatch`] does not include it,
/// because it is this screen's convention rather than the platform's.
///
/// Navigation is deliberately absent: `Up`, `Down`, `Left` and `Right` fall through to the list,
/// which owns the arithmetic and reports the result through the outbox. An arm for them here would
/// win — the bridge asks the app first — and would move the cursor without a viewport to move it in.
pub fn on_key(loading: bool, ev: KeyEvent) -> Option<Msg> {
    if ev.key == Key::Call {
        return Some(Msg::Open);
    }
    softkeys(loading).dispatch(ev)
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbian_ui::{Handled, Rect, Softkey, Theme, Uniform};

    use crate::chats::{ChatList, ChatListAction};
    use crate::model::{Media, Message};

    /// The content band, which is the viewport a list places itself in.
    fn viewport_h(theme: &Theme<'_>) -> i32 {
        symbian_ui::Frame::split(Rect::from_xywh(0, 0, 320, 240), theme, true, true).content.height()
    }

    /// The real font atlases, through the same loader the parity harness uses. A fake one-glyph
    /// atlas would do for the arithmetic below, and would be a second thing to keep in step with the
    /// fonts the screen actually draws in.
    fn with_theme(f: impl FnOnce(&Theme<'_>)) {
        let atlases = symbian_preview::Atlases::load();
        let mut ran = false;
        atlases.with_themes(|dark, _light| {
            f(dark);
            ran = true;
        });
        assert!(ran, "the atlases did not yield a theme");
    }

    // ---- the numbers ------------------------------------------------------------------------------

    #[test]
    fn the_metrics_here_are_the_metrics_the_theme_uses() {
        // The assumption this module stands on, asserted rather than argued: a view is built without
        // a theme, so the row height and the row inset come from `Metrics::default()` — and that is
        // only the same number as `theme.metrics` for as long as no theme carries its own. The day
        // one does, this fails here instead of a pixel at a time in a screenshot.
        with_theme(|t| {
            assert_eq!(pad(), t.metrics.pad);
            assert_eq!(row_height(), t.metrics.row_h);
        });
    }

    // ---- the projection ---------------------------------------------------------------------------

    #[test]
    fn a_row_carries_what_the_list_draws_and_nothing_else() {
        let store = Store::mock();
        let rows = rows(&store);
        assert_eq!(rows.len(), store.chats.len());
        for (row, chat) in rows.iter().zip(&store.chats) {
            assert_eq!(row.name, chat.name);
            assert_eq!(row.time, chat.time);
            assert_eq!(row.preview, chat.preview());
            assert_eq!(row.unread, chat.unread);
            assert_eq!(row.last_outgoing, chat.last_outgoing);
            // Derived once here rather than per visible row per frame.
            assert_eq!(row.initials, chat.initials());
            assert_eq!(row.color_seed, chat.color_seed());
        }
    }

    #[test]
    fn an_inline_photo_does_not_travel_into_the_row() {
        // The defect this projection exists for. The row closure used to capture `store.chats.clone()`
        // — every `Vec<Message>`, including whatever inline JPEG the top message carries — once per
        // rebuild, on a 4 MB heap. A row is seven small values, and this is what says so.
        let mut store = Store::mock();
        let heavy = alloc::vec![0xABu8; 64 * 1024];
        let chat = &mut store.chats[0];
        chat.messages.push(Message {
            id: 99,
            text: String::new(),
            outgoing: false,
            time: "14:32".into(),
            state: crate::model::Delivery::Read,
            media: Some(Media::Photo {
                id: 7,
                access_hash: 0,
                file_reference: alloc::vec::Vec::new(),
                dc_id: 2,
                thumb_size: "m".into(),
                size: heavy.len() as i64,
                preview: Some(heavy.clone()),
            }),
        });

        let row = &rows(&store)[0];
        // The media label, not the caption and certainly not the bytes.
        assert_eq!(row.preview, "Foto");
        assert!(row.preview.capacity() < 64, "the row is holding something it should not be");
    }

    #[test]
    fn projected_rows_are_shared_rather_than_copied() {
        // What makes a rebuild cheap: the app keeps the slice and every view after the first costs a
        // reference count. Two handles, one allocation.
        let store = Store::mock();
        let rows = rows(&store);
        let again = rows.clone();
        assert!(Rc::ptr_eq(&rows, &again));
        assert_eq!(Rc::strong_count(&rows), 2);
    }

    // ---- the keys ---------------------------------------------------------------------------------

    fn press(k: Key) -> KeyEvent {
        KeyEvent::new(k)
    }

    #[test]
    fn the_green_key_opens_the_highlighted_chat() {
        // `Softkeys::dispatch` does not know about `Key::Call` — it is this screen's convention, and
        // `ChatList::activate` has always honoured it. Losing it in the translation would be a key
        // that silently stopped working, which is the kind of thing nobody reports.
        assert_eq!(on_key(false, press(Key::Call)), Some(Msg::Open));
        assert_eq!(on_key(false, press(Key::Select)), Some(Msg::Open));
    }

    #[test]
    fn the_softkeys_mean_what_the_hand_written_screen_means() {
        assert_eq!(on_key(false, press(Key::Softkey(Softkey::Left))), Some(Msg::Options));
        assert_eq!(on_key(false, press(Key::Softkey(Softkey::Right))), Some(Msg::Quit));
        assert_eq!(softkeys(false).labels(), [Some("Opções"), Some("Abrir"), Some("Sair")]);
        // Mid-request, the left label is the only progress this screen shows.
        assert_eq!(softkeys(true).labels(), [Some("..."), Some("Abrir"), Some("Sair")]);
    }

    #[test]
    fn navigation_is_left_to_the_list() {
        // The bridge asks the app first, so an arm for `Down` here would win — and would move the
        // cursor without knowing how tall the band is, which is what a page key needs.
        for k in [Key::Up, Key::Down, Key::Left, Key::Right] {
            assert_eq!(on_key(false, press(k)), None, "{k:?} was claimed by the screen");
        }
    }

    // ---- the pagination condition -----------------------------------------------------------------

    /// What the hand-written screen does with Down, at this selection in a list of this length.
    fn hand_written_load_more(chats: usize, selected: usize, theme: &Theme<'_>) -> bool {
        let mut store = Store::mock();
        // A list of exactly `chats` rows, whatever the mock happens to hold.
        while store.chats.len() < chats {
            let c = store.chats[0].clone();
            store.chats.push(c);
        }
        store.chats.truncate(chats);

        let mut list = ChatList::new();
        let rows = Uniform { count: store.chats.len(), height: theme.metrics.row_h };
        let vp = viewport_h(theme);
        // Through `select`, which derives the scroll offset from the selection — the state the app is
        // actually in when the key arrives. Setting `selected` alone would compare against a cursor
        // outside its own viewport.
        list.state.select(selected, &rows, vp);
        let (handled, action) = list.handle_key(press(Key::Down), &store, theme, vp);
        assert_eq!(handled, Handled::Consumed);
        matches!(action, ChatListAction::LoadMore)
    }

    #[test]
    fn down_on_the_last_row_is_exactly_the_hand_written_pagination_condition() {
        // `chats.rs` asks for another page when the cursor is on the last row **and** the list is
        // scrolled to the bottom. This screen asks when the list reports `Edge::Bottom`, which is
        // Down with the cursor already on the last row — no scroll offset in sight.
        //
        // They are the same condition, and this is why: the offset is derived from the selection by
        // `ensure_visible`, which parks the last row against the bottom edge. On a list that
        // overflows, `scroll == content - viewport`; on one that fits, both sides are zero. There is
        // no state in which the cursor is on the last row and the list is not at the bottom.
        //
        // Checked across lengths that fit the screen (3), that fill it exactly, and that overflow it
        // (20) — because the two halves of the original condition only disagree, if they ever can,
        // where the content height crosses the viewport.
        with_theme(|t| {
            let fits = (viewport_h(t) / t.metrics.row_h) as usize;
            for n in [1usize, 3, fits, fits + 1, 20] {
                for selected in 0..n {
                    let ours = selected == n - 1;
                    assert_eq!(
                        ours,
                        hand_written_load_more(n, selected, t),
                        "{n} rows, cursor on {selected}"
                    );
                }
            }
        });
    }

    #[test]
    fn an_empty_list_never_asks_for_another_page() {
        // `chats.rs` guards on `!store.chats.is_empty()`; here there is no list to report an edge, so
        // the guard is structural — the empty screen has a placeholder where the list would be.
        with_theme(|t| {
            let store = Store { chats: alloc::vec::Vec::new(), ..Store::mock() };
            let mut list = ChatList::new();
            let (_, action) = list.handle_key(press(Key::Down), &store, t, viewport_h(t));
            assert!(matches!(action, ChatListAction::None));
        });
    }
}
