//! The chat list's Options menu, as a screen.
//!
//! ```text
//!   ┌────────────────────────────────────────────┐
//!   │ Opções                                     │
//!   │ ▌Atualizar                                 │
//!   │  Log: ligado                               │
//!   └────────────────────────────────────────────┘
//!     Escolher                          Voltar
//! ```
//!
//! A screen and not a popup, for the same reason the calendar's menu is one: a modal would be a
//! second way for a screen to exist, with its own focus rules, its own back key and its own reason
//! to disagree with the softkey bar. A list is what this toolkit already draws well.
//!
//! # Why the chat list grew a menu at all
//!
//! Its left softkey was "Atualizar", spending the one slot a Symbian screen has for *everything
//! else* on a single verb. The moment there was a second thing to offer — the run-time log switch —
//! the slot had to become Opções, which is the convention every other application on the phone
//! follows and the one the launcher and the calendar already use here.
//!
//! # The log entry carries its own state
//!
//! `Log: ligado` / `Log: desligado`, rather than a plain Log de depuração that toggles something
//! invisible. And selecting it does **not** close the menu, so the label the user just changed is
//! still on screen — the only feedback a switch like this can have.

use alloc::rc::Rc;
use alloc::vec::Vec;

use symbian_decl_ui::outbox::Outbox;
use symbian_decl_ui::slot::SlotTable;
use symbian_decl_ui::spacing::{Gap, Pad};
use symbian_decl_ui::widgets::{ListItem, Node, ScrollList, Screen};
use symbian_decl_ui::{KeyEvent, Softkeys};

/// What an entry does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Ask the server for the dialog list again — what the left softkey used to be.
    Refresh,
    /// Turn the device log on or off, now and for the next launch.
    DebugLog,
}

/// The entries, in order. A fixed list: this menu belongs to one screen, so there is nothing for it
/// to depend on yet — and when there is, this is where it goes.
pub fn entries() -> Vec<Action> {
    alloc::vec![Action::Refresh, Action::DebugLog]
}

/// An entry's label, with the state folded in for the entries that *are* a state.
pub fn label(action: Action, debug_on: bool) -> &'static str {
    match action {
        Action::Refresh => crate::strings::refresh(),
        Action::DebugLog if debug_on => "Log: ligado",
        Action::DebugLog => "Log: desligado",
    }
}

/// What the menu can ask the application to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// The cursor moved to this row.
    Select(usize),
    /// Carry out the highlighted entry.
    Run,
    /// Close the menu, changing nothing.
    Back,
}

/// The softkey bar: choose, or leave.
///
/// The middle slot is the action, which is where this hardware's thumb goes, and Voltar is the way
/// out. Nothing on the left: a menu opened from a menu is not a thing that can happen.
pub fn softkeys() -> Softkeys<Msg> {
    Softkeys::new().action(symbian_ui::strings::select(), Msg::Run).back(symbian_ui::strings::back(), Msg::Back)
}

/// What a key means before the list sees it.
///
/// Navigation is deliberately absent — `Up`/`Down` fall through to the `ScrollList`, which owns the
/// arithmetic and reports where the cursor went. An arm for them here would win, because the bridge
/// asks the app first, and would move a cursor with no viewport behind it.
pub fn on_key(ev: KeyEvent) -> Option<Msg> {
    softkeys().dispatch(ev)
}

/// Build the menu screen.
pub fn view(selected: usize, debug_on: bool, out: &Outbox<Msg>, slots: &mut SlotTable) -> Node {
    let labels: Rc<[&'static str]> =
        entries().into_iter().map(|a| label(a, debug_on)).collect::<Vec<_>>().into();
    let count = labels.len();
    let moved = out.clone();
    Node::leaf(
        Screen::new()
            .title(symbian_ui::strings::options())
            .content(
                ScrollList::new(slots, count, row_height())
                    .selected(selected)
                    .on_move(move |i| moved.push(Msg::Select(i)))
                    .row(move |i, is_selected| {
                        // `ListItem` rather than a `Row` of one `Text`, and the reason is not
                        // tidiness. Written by hand this row was missing `CrossAlign::Stretch`, so
                        // the text took its own 17-pixel height at the *top* of a 38-pixel band:
                        // measured on the sheet, three pixels of air above the line and twenty-five
                        // below it. That is the exact defect `list_item.rs` was written to stop
                        // being written again, and it survived here because this screen had no
                        // preview sheet — it has one now.
                        //
                        // `.plain()` keeps the body weight this menu had; `ListItem` defaults to
                        // strong. The ink is no longer computed here: `ListItem` resolves
                        // `Selection`/`Text` from `selected` itself, which is the same two values
                        // this closure was picking between.
                        ListItem::new(labels[i])
                            .plain()
                            .selected(is_selected)
                            // The list's own inset, not the role default: this menu's rows are
                            // meant to line up with the dialog list behind it, which is what
                            // `pad()` deferring to `chats_decl` says.
                            .pad(Pad::xy(Gap::Exact(pad()), Gap::None))
                            .build()
                    }),
            )
            .softkeys(softkeys()),
    )
}

/// The row's inset, the same as the dialog list's.
fn pad() -> i32 {
    crate::chats_decl::pad()
}

/// Row height: the same number every other list in this app uses, so the menu's rows line up with
/// the dialog list behind it.
fn row_height() -> i32 {
    crate::chats_decl::row_height()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_entry_reports_its_own_state() {
        assert_eq!(label(Action::DebugLog, true), "Log: ligado");
        assert_eq!(label(Action::DebugLog, false), "Log: desligado");
        assert_eq!(label(Action::Refresh, true), crate::strings::refresh(), "not every entry is a state");
    }

    /// Refresh stays first: it is what the softkey used to do, and a user who pressed the left key
    /// out of habit lands on it with the cursor already there.
    #[test]
    fn refresh_is_the_first_entry() {
        assert_eq!(entries().first().copied(), Some(Action::Refresh));
    }

    /// The row's line must sit in the middle of its band, not at the top of it.
    ///
    /// This is a regression test with a rendered defect behind it. Written by hand, the row was a
    /// `Row::new()` — whose `CrossAlign` defaults to `Start` — holding one `Text`, so the line took
    /// its own height and hung from the top of the band: **three** pixels of air above it and
    /// **twenty-five** below, measured on `preview-out/17-menu.png`. Nothing caught it because this
    /// screen had no preview sheet and no parity scene; it is the defect `symbian_decl_ui`'s
    /// `list_item` module documents as "the single difference a pixel-for-pixel comparison ever
    /// found", reappearing in an application that had the component available and did not use it.
    ///
    /// The assertion is on the *balance* rather than on an absolute position, because the absolute
    /// one moves with the font and the metrics and this is not a typography test. A centred line
    /// leaves the same air above and below to within the difference between an ascender and a
    /// descender; a top-anchored one leaves eight times as much below as above.
    #[test]
    fn a_menu_row_sits_in_the_middle_of_its_band_and_not_at_the_top() {
        use symbian_gfx::{Rect, E72_SCREEN};
        use symbian_preview::{Atlases, Sheet};
        use symbian_ui::{App as _, Key, Softkey};

        let (w, h) = (E72_SCREEN.w, E72_SCREEN.h);
        // The real atlases, not `symbian_ui::testing`: that one holds a single glyph, so most
        // strings draw nothing under it and a test about where a line of text *sits* would be
        // measuring an empty band. This is the same font chain the device links and the same one
        // `examples/preview.rs` renders with.
        let px = Atlases::load().with_themes(|t, _light| {
            let mut app = crate::mvu::mock();
            // Drawn before the key so the walk has rects to answer at, then again after it — the
            // reason every scene in `preview.rs` warms a frame per press.
            let mut warm = Sheet::new(E72_SCREEN);
            app.draw(&mut warm.canvas(), t);
            app.handle_key(KeyEvent::new(Key::Softkey(Softkey::Left)), t, Rect::from_size(E72_SCREEN));
            let mut s = Sheet::new(E72_SCREEN);
            app.draw(&mut s.canvas(), t);
            s.pixels().to_vec()
        });
        let at = |x: i32, y: i32| px[(y * w + x) as usize];
        // The page, sampled where nothing is ever drawn: below the last entry, above the softkeys.
        let page = at(w - 8, h - 40);

        // The selection band, found by its full width: the longest contiguous run of rows that are
        // not the page at the far right edge. The title bar above it is shorter than a row.
        let lit: Vec<i32> = (0..h - 40).filter(|&y| at(w - 8, y) != page).collect();
        let (mut best, mut i) = ((0usize, 0i32), 0usize);
        while i < lit.len() {
            let mut j = i;
            while j + 1 < lit.len() && lit[j + 1] == lit[j] + 1 {
                j += 1;
            }
            if j - i + 1 > best.0 {
                best = (j - i + 1, lit[i]);
            }
            i = j + 1;
        }
        let (run, start) = best;
        assert!(run as i32 >= row_height(), "no selection band on screen: run of {run}");
        // The band is the run's last `row_height()` rows — the title bar sits directly above it and
        // joins the run, because the selected entry is the first one.
        let top = start + run as i32 - row_height();

        // The line's ink, against the band it is drawn on — sampled **per row**, at the right edge
        // where no text reaches. The band is a vertical gradient, so one colour taken from its
        // middle differs from almost every row of it, and a comparison against that single sample
        // would call every row "inked" and report a perfectly centred line whatever was drawn. That
        // is not hypothetical: it is what the first version of this test did, and it stayed green
        // with the defective row put back.
        let ys: Vec<i32> = (top..top + row_height())
            .filter(|&y| (4..w - 12).any(|x| at(x, y) != at(w - 8, y)))
            .collect();
        assert!(!ys.is_empty(), "the row drew no text at all");
        let (above, below) = (ys[0] - top, top + row_height() - 1 - ys[ys.len() - 1]);
        assert!(
            (above - below).abs() <= 4,
            "the line is not centred in its band: {above} pixels above, {below} below",
        );
    }

    #[test]
    fn the_bar_offers_a_choice_and_a_way_out() {
        // Read from the same tables the bar reads. A test that spelled the words would be
        // testing the translation — which has its own test, in `strings.rs` — and would fail on
        // every phone whose language is not the one it was written on.
        assert_eq!(
            softkeys().labels(),
            [None, Some(symbian_ui::strings::select()), Some(symbian_ui::strings::back())]
        );
    }
}
