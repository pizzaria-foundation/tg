//! The photo viewer, drawn twice, compared pixel for pixel.
//!
//! ```text
//! cargo run -p tg --example viewer_parity      # → parity-out/ and a report
//! cargo test -p tg --example viewer_parity     # the assertion
//! ```
//!
//! `symbian_ui::Viewer::draw` is the shipping screen: chrome and image in one call. `tg::viewer_decl`
//! declares the chrome and lets the layout hand the image its band. The scenes are aimed at the
//! branches in the blit — an image smaller than the band, one larger than it, one panned, one panned
//! past its own edge, and one with no pixels at all, which must draw nothing rather than panic.
//!
//! The image is generated rather than loaded: a gradient with a one-pixel border, so a photo drawn two
//! pixels off shows up as a border that is not where it should be rather than as a wash that looks
//! plausible either way.

use std::process::ExitCode;

use symbian_decl_ui::layout;
use symbian_decl_ui::outbox::Outbox;
use symbian_decl_ui::slot::SlotTable;
use symbian_gfx::{Canvas, Rect, E72_SCREEN};
use symbian_preview::{Atlases, Parity};
use symbian_ui::{Key, KeyEvent, Size, Theme, Viewer};

const OUT: &str = "parity-out";

fn main() -> ExitCode {
    let atlases = Atlases::load();
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

/// One scene: an image, and how far the user has panned into it.
struct Scene {
    name: &'static str,
    size: Size,
    /// Pan keys pressed, in order.
    keys: &'static [Key],
}

fn scenes() -> Vec<Scene> {
    vec![
        // Smaller than the band in both axes: centred, and panning must do nothing at all.
        Scene { name: "viewer-small", size: Size::new(120, 90), keys: &[] },
        Scene { name: "viewer-small-panned", size: Size::new(120, 90), keys: &[Key::Down, Key::Right] },
        // Taller and wider than the band: the top-left corner, then panned into.
        Scene { name: "viewer-large", size: Size::new(640, 480), keys: &[] },
        Scene { name: "viewer-large-panned", size: Size::new(640, 480), keys: &[Key::Down, Key::Right] },
        // Past the far edge, which is where panning and drawing used to disagree: the scroll clamped
        // against the screen while the blit clipped to the box, so the bottom was reachable and never
        // visible.
        Scene {
            name: "viewer-large-clamped",
            size: Size::new(640, 480),
            keys: &[Key::Down, Key::Down, Key::Down, Key::Down, Key::Down, Key::Down, Key::Down, Key::Right, Key::Right, Key::Right, Key::Right, Key::Right, Key::Right, Key::Right],
        },
        // One axis over, one under: the narrow axis centres while the tall one pans.
        Scene { name: "viewer-tall", size: Size::new(160, 900), keys: &[Key::Down] },
        // A decode that produced nothing. Must draw nothing rather than panic — a panic on this
        // device is a silent vanish.
        Scene { name: "viewer-empty", size: Size::new(0, 0), keys: &[] },
    ]
}

/// A gradient with a one-pixel border, so an image drawn in the wrong place says so.
fn image(size: Size) -> Vec<u16> {
    let (w, h) = (size.w.max(0), size.h.max(0));
    let mut px = vec![0u16; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
            let v = if edge {
                0xFFFF
            } else {
                // Five bits of red, six of green: a diagonal ramp that is different at every corner.
                let r = (x * 31 / w.max(1)) as u16;
                let g = (y * 63 / h.max(1)) as u16;
                (r << 11) | (g << 5)
            };
            px[(y * w + x) as usize] = v;
        }
    }
    px
}

fn viewer_for(scene: &Scene, theme: &Theme<'_>) -> Viewer {
    let mut v = Viewer::new(image(scene.size), scene.size);
    let area = Viewer::content(Rect::from_size(E72_SCREEN), theme);
    for k in scene.keys {
        v.handle_key(KeyEvent::new(*k), area);
    }
    v
}

/// The shipping screen: chrome and image in one call.
fn render_by_hand(c: &mut Canvas<'_>, scene: &Scene, theme: &Theme<'_>) {
    viewer_for(scene, theme).draw(c, theme, "Foto", "Voltar");
}

/// The declarative screen, through the real layout pass.
fn render_declared(c: &mut Canvas<'_>, scene: &Scene, theme: &Theme<'_>) {
    let shared = std::rc::Rc::new(std::cell::RefCell::new(viewer_for(scene, theme)));
    let out = Outbox::new();
    let mut slots = SlotTable::new();
    let mut cache = symbian_decl_ui::UiCache::new();
    for _ in 0..2 {
        slots.begin_frame();
        let tree = tg::viewer_decl::view(&shared, &out);
        layout::draw_frame(&tree, Rect::from_size(E72_SCREEN), &mut cache, c, theme);
        slots.end_frame();
    }
}

fn run(p: &mut Parity, dark: &Theme<'_>, light: &Theme<'_>) {
    for scene in scenes() {
        p.check(
            scene.name,
            dark,
            |c| render_by_hand(c, &scene, dark),
            |c| render_declared(c, &scene, dark),
        );
    }
    // One scene in the light palette: the chrome is what changes colour, and the image is the same
    // bytes either way.
    let scene = &scenes()[2];
    p.check(
        "viewer-large-light",
        light,
        |c| render_by_hand(c, scene, light),
        |c| render_declared(c, scene, light),
    );
}

#[test]
fn the_declared_viewer_is_the_hand_written_one() {
    let atlases = Atlases::load();
    let mut p = Parity::new(OUT).keep_matching(true);
    atlases.with_themes(|dark, light| {
        run(&mut p, dark, light);
    });
    assert_eq!(p.checked(), 8, "a scene stopped being compared");
    p.finish();
}

#[test]
fn every_scene_renders_something_different_except_the_one_that_must_not() {
    // Ten comparisons of one state would pass and prove one thing, so each scene's pixels are checked
    // against every earlier scene's. One pair is deliberately exempt and asserted the other way
    // round: an image smaller than the band cannot pan, so pressing Down and Right on it *must*
    // change nothing. That is the clamp working, and it is worth saying out loud rather than leaving
    // as a hole in the list.
    let atlases = Atlases::load();
    atlases.with_themes(|dark, _light| {
        let shot = |scene: &Scene| {
            let mut sheet = symbian_preview::Sheet::new(E72_SCREEN);
            render_by_hand(&mut sheet.canvas(), scene, dark);
            sheet.pixels().to_vec()
        };
        let all = scenes();
        let small = shot(&all[0]);
        let small_panned = shot(&all[1]);
        assert_eq!(small, small_panned, "an image smaller than the band panned anyway");

        let mut seen: Vec<(&str, Vec<u16>)> = Vec::new();
        for scene in all.iter().filter(|s| s.name != "viewer-small-panned") {
            let px = shot(scene);
            for (name, other) in &seen {
                assert_ne!(&px, other, "{} drew the same pixels as {name}", scene.name);
            }
            seen.push((scene.name, px));
        }
    });
}
