//! The three login screens, drawn twice, compared pixel for pixel.
//!
//! ```text
//! cargo run -p tg --example login_parity      # → parity-out/ and a report
//! cargo test -p tg --example login_parity     # the assertion
//! ```
//!
//! [`tg::login`] places rects; [`tg::login_decl`] describes a tree. Both are handed the same state
//! and the same theme, and the buffers must match. The hand-written screen is the reference because
//! it is what ships — where the two differ, read the difference before adjusting either.
//!
//! # The scenes, and the branch each one is aimed at
//!
//! The dialog list taught this: the first comparison in this repository rendered one frame and
//! reported "identical" while every other state was unproven. So, per screen and both themes where
//! the palette is the only difference:
//!
//! * **phone**, empty and typed, with the error line, and with `credentials_missing` — which
//!   replaces the error line rather than sitting beside it;
//! * **phone disconnected**, because the middle softkey disappears rather than greying out;
//! * **code**, with and without the digit count the server sent, and with a status along the bottom
//!   — the two bottom lines are the reason `Stack` exists and the one place the layers could shift;
//! * **password**, masked and revealed, with a hint;
//! * **a selection**, because the field paints one behind the text and a screen that lost it would
//!   replace text the user did not know was chosen;
//! * **waiting**, which is a different screen and not this one with the field hidden.
//!
//! # What this comparison cannot see
//!
//! The unlabelled key. `login.rs`'s phone screen answers `Softkey::Right` with nothing on the bar,
//! and a pixel comparison is blind to a key that draws nothing — see `login_decl::on_key`, which
//! preserves it deliberately and says why.

use std::process::ExitCode;

use symbian_decl_ui::layout;
use symbian_decl_ui::slot::SlotTable;
use symbian_gfx::{Canvas, Rect, E72_SCREEN};
use symbian_preview::{Atlases, Parity};
use symbian_ui::{edit, Theme};

use tg::login::{Login, Screen as Imperative};
use tg::login_decl::{State, Which};

const OUT: &str = "parity-out";

fn main() -> ExitCode {
    let atlases = Atlases::load();
    // `keep_matching`, so a run that reports "identical" still leaves the pictures behind. A
    // comparison whose output only exists when it fails is one nobody can look at when it passes,
    // and looking at it is how the missing eye beside the password field was found — the numbers
    // agreed before the eye existed, because neither side drew one.
    let mut p = Parity::new(OUT).keep_matching(true);
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

/// One scene: what is on screen, in both forms.
struct Scene {
    name: &'static str,
    /// What the field holds, and what is selected in it.
    typed: &'static str,
    select_all: bool,
    state: State,
}

fn scenes() -> Vec<Scene> {
    let connected = |which: Which| State {
        which,
        error: None,
        status: String::new(),
        connected: true,
    };

    vec![
        Scene {
            name: "login-phone-empty",
            typed: "",
            select_all: false,
            state: connected(Which::Phone { credentials_missing: false }),
        },
        Scene {
            name: "login-phone-typed",
            typed: "11999990000",
            select_all: false,
            state: connected(Which::Phone { credentials_missing: false }),
        },
        Scene {
            name: "login-phone-selection",
            typed: "11999990000",
            select_all: true,
            state: connected(Which::Phone { credentials_missing: false }),
        },
        Scene {
            name: "login-phone-error",
            typed: "1199",
            select_all: false,
            state: State {
                error: Some("PHONE_NUMBER_INVALID".into()),
                ..connected(Which::Phone { credentials_missing: false })
            },
        },
        Scene {
            name: "login-phone-no-credentials",
            typed: "",
            select_all: false,
            state: connected(Which::Phone { credentials_missing: true }),
        },
        Scene {
            name: "login-phone-disconnected",
            typed: "1199",
            select_all: false,
            state: State {
                connected: false,
                ..connected(Which::Phone { credentials_missing: false })
            },
        },
        Scene {
            name: "login-code-no-length",
            typed: "",
            select_all: false,
            state: connected(Which::Code { length: None }),
        },
        Scene {
            name: "login-code-typed",
            typed: "12345",
            select_all: false,
            state: connected(Which::Code { length: Some(5) }),
        },
        Scene {
            name: "login-code-status",
            typed: "123",
            select_all: false,
            state: State {
                status: "conectando…".into(),
                ..connected(Which::Code { length: Some(5) })
            },
        },
        Scene {
            name: "login-code-error-and-status",
            typed: "12345",
            select_all: false,
            state: State {
                error: Some("PHONE_CODE_INVALID".into()),
                status: "reconectando…".into(),
                ..connected(Which::Code { length: Some(6) })
            },
        },
        Scene {
            name: "login-password-masked",
            typed: "hunter2",
            select_all: false,
            state: connected(Which::Password { hint: "a dica do servidor".into(), masked: true }),
        },
        Scene {
            name: "login-password-revealed",
            typed: "hunter2",
            select_all: false,
            state: connected(Which::Password { hint: "a dica do servidor".into(), masked: false }),
        },
        Scene {
            name: "login-password-no-hint",
            typed: "hunter2",
            select_all: false,
            state: connected(Which::Password { hint: String::new(), masked: true }),
        },
        Scene {
            name: "login-waiting",
            typed: "",
            select_all: false,
            state: connected(Which::Waiting("conectando…".into())),
        },
    ]
}

/// Every scene in dark, and the ones whose colours differ most in light as well.
fn run(p: &mut Parity, dark: &Theme<'_>, light: &Theme<'_>) {
    for scene in scenes() {
        p.check(
            scene.name,
            dark,
            |c| render_by_hand(c, &scene, dark),
            |c| render_declared(c, &scene, dark),
        );
    }
    for scene in scenes() {
        // The light palette over one screen of each kind: the layout is the same arithmetic, so what
        // a second full pass would prove is that `Ink` resolves — and one scene per screen proves
        // that with three comparisons instead of fourteen.
        if !matches!(
            scene.name,
            "login-phone-error" | "login-password-revealed" | "login-waiting"
        ) {
            continue;
        }
        let name: &'static str = match scene.name {
            "login-phone-error" => "login-phone-error-light",
            "login-password-revealed" => "login-password-revealed-light",
            _ => "login-waiting-light",
        };
        p.check(
            name,
            light,
            |c| render_by_hand(c, &scene, light),
            |c| render_declared(c, &scene, light),
        );
    }
}

/// A field with this scene's contents, mask and selection.
fn field(scene: &Scene, masked: bool) -> edit::TextField {
    let mut f = match &scene.state.which {
        // The digits filter lives in the buffer, so the two sides must build the same buffer or a
        // pasted letter would appear on one screen and not the other.
        Which::Phone { .. } | Which::Code { .. } => tg::login::digits_field(16),
        _ => edit::TextField::with_limit(128),
    };
    f.set_masked(masked);
    f.insert_str(scene.typed);
    if scene.select_all {
        f.select_all();
    }
    f
}

/// The shipping screen.
fn render_by_hand(c: &mut Canvas<'_>, scene: &Scene, theme: &Theme<'_>) {
    let masked = matches!(scene.state.which, Which::Password { masked: true, .. });
    let f = field(scene, masked);
    let error = scene.state.error.clone();
    // The hand-written screen holds the same shared buffer type the declarative one does — one field
    // behind an `Rc`, since the application has to be able to read it from `update`.
    let f = tg::login::shared(f);
    let screen = match &scene.state.which {
        Which::Phone { .. } => Imperative::Phone { field: f, error },
        Which::Code { length } => Imperative::Code { field: f, length: *length, error },
        Which::Password { hint, .. } => {
            Imperative::Password { field: f, hint: hint.clone(), error }
        }
        Which::Waiting(msg) => Imperative::Waiting(leak(msg)),
    };
    // `for_preview` is the one with no credentials, which is what that scene is about; every other
    // scene wants a build that could log in.
    let mut login = if matches!(scene.state.which, Which::Phone { credentials_missing: true }) {
        Login::for_preview(screen)
    } else {
        Login::for_preview_with_credentials(screen)
    };
    login.set_connected(scene.state.connected);
    // Not on the waiting screen: `set_status` also rewrites what a `Waiting` says, which is what it
    // is for on the real screen and would silently replace this scene's message with the status.
    if !matches!(scene.state.which, Which::Waiting(_)) {
        login.set_status(leak(&scene.state.status));
    }
    login.draw(c, theme);
}

/// The declarative screen, through the real layout pass.
///
/// Two frames, and the second is the one compared — the first fills the measure cache, which is the
/// steady state a device is always in by the time anyone looks at the screen.
fn render_declared(c: &mut Canvas<'_>, scene: &Scene, theme: &Theme<'_>) {
    let masked = matches!(scene.state.which, Which::Password { masked: true, .. });
    let buffer = std::rc::Rc::new(std::cell::RefCell::new(field(scene, masked)));
    let mut slots = SlotTable::new();
    let mut cache = symbian_decl_ui::UiCache::new();
    for _ in 0..2 {
        slots.begin_frame();
        let tree = tg::login_decl::view(&scene.state, &buffer, &mut slots);
        layout::draw_frame(&tree, Rect::from_size(E72_SCREEN), &mut cache, c, theme);
        slots.end_frame();
    }
}

/// `Screen::Waiting` and the status line hold `&'static str`, which the hand-written screen chose
/// because the messages are literals. A scene builds its text at runtime, so it is leaked — this is
/// a comparison harness that runs once and exits, and the alternative is changing a shipping type to
/// suit a test.
fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

#[test]
fn the_declared_login_screens_are_the_hand_written_ones() {
    let atlases = Atlases::load();
    let mut p = Parity::new(OUT);
    atlases.with_themes(|dark, light| {
        run(&mut p, dark, light);
    });
    // The count is asserted as well as the diffs: a refactor that stops building scenes would
    // otherwise turn this into a green light for nothing.
    assert_eq!(p.checked(), 17, "a scene stopped being compared");
    p.finish();
}

/// The scenes are distinct, which is what makes seventeen of them worth more than one.
#[test]
fn every_scene_renders_something_different() {
    let atlases = Atlases::load();
    atlases.with_themes(|dark, _light| {
        let shot = |scene: &Scene| {
            let mut sheet = symbian_preview::Sheet::new(E72_SCREEN);
            render_by_hand(&mut sheet.canvas(), scene, dark);
            sheet.pixels().to_vec()
        };
        let all = scenes();
        let mut seen: Vec<(&str, Vec<u16>)> = Vec::new();
        for scene in &all {
            let px = shot(scene);
            for (name, other) in &seen {
                assert_ne!(&px, other, "{} drew the same pixels as {name}", scene.name);
            }
            seen.push((scene.name, px));
        }
    });
}
