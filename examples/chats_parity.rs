//! The dialog list, drawn twice, compared pixel for pixel — in every state that has a state.
//!
//! ```text
//! cargo run -p tg --example chats_parity      # → parity-out/ and a report
//! cargo test -p tg --example chats_parity     # the assertion
//! ```
//!
//! [`tg::chats`] places rects; [`tg::chats_decl`] describes a tree. Both are handed the same store
//! and the same theme, and the buffers must match. This is the only evidence that the declarative
//! rewrite of a real screen changed nothing, and "it looks about right" is not that evidence — a row
//! two pixels high in the wrong place survives a glance and not a diff.
//!
//! # Why there are nine scenes and not one
//!
//! Because the first version of this file had one, and reported "identical" — one store, dark
//! theme, selection zero, scroll zero. Everything the single scene did not render was unproven.
//!
//! The scenes below are chosen against the *branches*, not against the look: a selection that is not
//! the first row, a list scrolled far enough to clip one, an empty list, a list still loading, a list
//! too short for a scrollbar, an unread count past the "999+" cutoff, and both themes. A scene per
//! branch is the difference between a comparison and a screenshot.
//!
//! # The scrollbar gutter, which is where the two nearly disagree
//!
//! `chats.rs` asks for the gutter with `scrollbar_gutter(theme, bar.is_some())`; `chats_decl.rs`
//! says `.scrollbar(true)` and gets it unconditionally. They agree today for one reason:
//! `chrome::scrollbar_gutter` **ignores** its `needed` argument and always reserves the width. So
//! the two screens are identical by accident rather than by agreement, and the day that function
//! starts honouring the flag, every row on a short list moves on one side only.
//!
//! That is why `chats-short-no-scrollbar` is in the list even though it passes: it is not testing a
//! bug, it is standing in front of one. A scene that passes today and would fail the moment someone
//! makes a reasonable-looking change is exactly what a regression suite is for.
//!
//! # Read the difference before changing either side
//!
//! The hand-written screen is the reference because it is what ships, and a reference is not a proof
//! of correctness. Nudging the declarative side until the numbers agree proves only that two things
//! can be made identical, which nobody doubted.

use std::process::ExitCode;

use symbian_decl_ui::layout;
use symbian_decl_ui::outbox::Outbox;
use symbian_decl_ui::slot::SlotTable;
use symbian_gfx::{Canvas, Rect, E72_SCREEN};
use symbian_preview::{Atlases, Parity};
use symbian_ui::{Theme, Uniform};

use tg::model::Store;

const OUT: &str = "parity-out";

fn main() -> ExitCode {
    let atlases = Atlases::load();
    let mut p = Parity::new(OUT);
    atlases.with_themes(|dark, light| {
        run(&mut p, dark, light);
    });
    println!("{}", p.report());
    if p.diffs().is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Every scene, against both implementations.
fn run(p: &mut Parity, dark: &Theme<'_>, light: &Theme<'_>) {
    let full = Store::mock();

    // The states the single-scene version never rendered, each one aimed at a branch.
    let empty = Store { chats: Vec::new(), ..Store::mock() };
    let loading = Store { dialogs_loading: true, ..Store::mock() };
    let short = Store { chats: full.chats.iter().take(3).cloned().collect(), ..Store::mock() };
    let noisy = {
        let mut s = Store::mock();
        if let Some(c) = s.chats.first_mut() {
            c.unread = 1234;
        }
        s
    };

    // (name, store, selected, theme)
    let scenes: [(&str, &Store, usize, &Theme<'_>); 9] = [
        ("chats-dark", &full, 0, dark),
        ("chats-selected", &full, 3, dark),
        ("chats-scrolled", &full, full.chats.len() - 1, dark),
        ("chats-empty", &empty, 0, dark),
        ("chats-loading", &loading, 0, dark),
        ("chats-short-no-scrollbar", &short, 0, dark),
        ("chats-unread-over-999", &noisy, 0, dark),
        ("chats-light", &full, 0, light),
        ("chats-light-selected", &full, 3, light),
    ];

    for (name, store, selected, theme) in scenes {
        p.check(
            name,
            theme,
            |c| render_by_hand(c, store, selected, theme),
            |c| render_declared(c, store, selected, theme),
        );
    }
}

/// The shipping screen, at the selection the scene asks for.
///
/// The selection goes in through `ListState::select`, which is what derives the scroll offset from
/// it — the same call the key handler makes. Setting `selected` alone would compare a screen with a
/// cursor off the bottom of its own viewport, which is not a state the app can be in.
fn render_by_hand(c: &mut Canvas<'_>, store: &Store, selected: usize, theme: &Theme<'_>) {
    let mut list = tg::chats::ChatList::new();
    let rows = Uniform { count: store.chats.len(), height: theme.metrics.row_h };
    list.state.select(selected, &rows, viewport_h(theme));
    list.draw(c, store, theme);
}

/// The declarative screen, through the real layout pass.
///
/// Two frames, and the second is the one compared: the first is what fills the measure cache and
/// lets the list derive its scroll offset from the selection. That is not a workaround — it is the
/// steady state, since a screen on a device is never on its first frame by the time anyone looks.
fn render_declared(c: &mut Canvas<'_>, store: &Store, selected: usize, theme: &Theme<'_>) {
    let mut slots = SlotTable::new();
    let mut cache = symbian_decl_ui::UiCache::new();
    // The outbox is where the list reports a moved cursor and a request for another page. Nothing
    // reads it here: this comparison is about pixels, and a screen drawn twice produces no keys.
    let out = Outbox::new();
    for _ in 0..2 {
        slots.begin_frame();
        let tree = tg::chats_decl::view(
            tg::chats_decl::rows(store),
            store.dialogs_loading,
            &store.status,
            selected,
            &out,
            &mut slots,
        );
        layout::draw_frame(&tree, Rect::from_size(E72_SCREEN), &mut cache, c, theme);
        slots.end_frame();
    }
}

/// The height of the content band: what a list has to place itself in.
fn viewport_h(theme: &Theme<'_>) -> i32 {
    symbian_ui::Frame::split(Rect::from_size(E72_SCREEN), theme, true, true).content.height()
}

#[test]
fn the_declared_dialog_list_is_the_hand_written_one() {
    let atlases = Atlases::load();
    let mut p = Parity::new(OUT);
    atlases.with_themes(|dark, light| {
        run(&mut p, dark, light);
    });
    // The count is asserted as well as the diffs: a refactor that stops building scenes would
    // otherwise turn this into a green light for nothing.
    assert_eq!(p.checked(), 9, "a scene stopped being compared");
    p.finish();
}

/// The scenes are distinct, which is what makes nine of them worth more than one.
///
/// A parameter that never reaches the render would produce nine copies of the same comparison, all
/// passing, all proving one thing. This renders the shipping screen alone and asserts that each
/// scene's inputs actually change its pixels — the instrument checking that it is pointed at
/// something.
#[test]
fn every_scene_renders_something_different() {
    let atlases = Atlases::load();
    atlases.with_themes(|dark, light| {
        let full = Store::mock();
        let empty = Store { chats: Vec::new(), ..Store::mock() };
        let loading = Store { dialogs_loading: true, ..Store::mock() };
        let short = Store { chats: full.chats.iter().take(3).cloned().collect(), ..Store::mock() };

        let shot = |store: &Store, selected: usize, theme: &Theme<'_>| {
            let mut sheet = symbian_preview::Sheet::new(E72_SCREEN);
            render_by_hand(&mut sheet.canvas(), store, selected, theme);
            sheet.pixels().to_vec()
        };

        let base = shot(&full, 0, dark);
        for (what, other) in [
            ("a different selection", shot(&full, 3, dark)),
            ("a scrolled list", shot(&full, full.chats.len() - 1, dark)),
            ("an empty list", shot(&empty, 0, dark)),
            ("a loading list", shot(&loading, 0, dark)),
            ("a short list", shot(&short, 0, dark)),
            ("the light theme", shot(&full, 0, light)),
        ] {
            assert_ne!(base, other, "{what} drew the same pixels as the base scene");
        }
    });
}
