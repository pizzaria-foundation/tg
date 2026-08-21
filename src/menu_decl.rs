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
//! the slot had to become "Opções", which is the convention every other application on the phone
//! follows and the one the launcher and the calendar already use here.
//!
//! # The log entry carries its own state
//!
//! `Log: ligado` / `Log: desligado`, rather than a plain "Log de depuração" that toggles something
//! invisible. And selecting it does **not** close the menu, so the label the user just changed is
//! still on screen — the only feedback a switch like this can have.

use alloc::rc::Rc;
use alloc::vec::Vec;

use symbian_decl_ui::outbox::Outbox;
use symbian_decl_ui::slot::SlotTable;
use symbian_decl_ui::widgets::text::{Ink, Text};
use symbian_decl_ui::widgets::{Node, Row, ScrollList, Screen};
use symbian_decl_ui::{KeyEvent, Softkeys};
use symbian_ui::gfx::Edges;

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
        Action::Refresh => "Atualizar",
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
/// The middle slot is the action, which is where this hardware's thumb goes, and "Voltar" is the way
/// out. Nothing on the left: a menu opened from a menu is not a thing that can happen.
pub fn softkeys() -> Softkeys<Msg> {
    Softkeys::new().action("Escolher", Msg::Run).back("Voltar", Msg::Back)
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
            .title("Opções")
            .content(
                ScrollList::new(slots, count, row_height())
                    .selected(selected)
                    .on_move(move |i| moved.push(Msg::Select(i)))
                    .row(move |i, is_selected| {
                        let ink = if is_selected { Ink::Selection } else { Ink::Text };
                        Node::leaf(
                            Row::new()
                                .padding(Edges::xy(pad(), 0))
                                .child(Text::new(labels[i]).ink(ink).flex(1)),
                        )
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
        assert_eq!(label(Action::Refresh, true), "Atualizar", "not every entry is a state");
    }

    /// Refresh stays first: it is what the softkey used to do, and a user who pressed the left key
    /// out of habit lands on it with the cursor already there.
    #[test]
    fn refresh_is_the_first_entry() {
        assert_eq!(entries().first().copied(), Some(Action::Refresh));
    }

    #[test]
    fn the_bar_offers_a_choice_and_a_way_out() {
        assert_eq!(softkeys().labels(), [None, Some("Escolher"), Some("Voltar")]);
    }
}
