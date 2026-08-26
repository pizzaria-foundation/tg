//! The photo viewer, described: a title, a way out, and the image between them.
//!
//! The smallest of the migrated screens, and the one that says what the shape is *for*. The viewer
//! itself is a toolkit widget — `symbian_ui::Viewer` — which pans with the D-pad, clamps at the edges
//! and blits once per frame. None of that is layout. What the hand-written screen added around it was
//! furniture: a title bar reading "Foto" and a right softkey reading Voltar, both drawn by
//! `Viewer::draw` because there was nowhere else to put them.
//!
//! Here they are a declaration, and `Viewer::draw_image` draws the pixels into the band the layout
//! produced. The same two-pixel inset, from the same function, so panning still clamps against the
//! rectangle the image is drawn in — which is the bug `Viewer::content` was extracted for.

use alloc::rc::Rc;
use core::cell::RefCell;

use symbian_decl_ui::outbox::Outbox;
use symbian_decl_ui::widget::{hash_i32, KeyCtx, Widget, WidgetHash};
use symbian_decl_ui::widgets::Screen;
use symbian_decl_ui::{Constraints, Handled, KeyEvent, Node, Softkeys};
use symbian_ui::{Canvas, Rect, Size, Theme, Viewer, ViewerAction};

/// A viewer both the application and its screen can hold.
pub type Shared = Rc<RefCell<Viewer>>;

/// What the viewer can ask the application to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// Leave the photo. Where *to* is the application's business — the viewer does not know what it
    /// was opened from, which is the toolkit widget's own rule and worth keeping.
    Back,
}

/// The softkey bar: one way out, and no action.
///
/// The middle slot stays empty on purpose. `Viewer::handle_key` treats `Softkey::Middle` as Back as
/// well, and with nothing labelled there `Screen` does not claim it — so it reaches the image widget,
/// which answers exactly as the hand-written screen did. A label would have been a second name for
/// the same key.
pub fn softkeys() -> Softkeys<Msg> {
    Softkeys::new().back(symbian_ui::strings::back(), Msg::Back)
}

/// What a key means before the image sees it.
pub fn on_key(ev: KeyEvent) -> Option<Msg> {
    softkeys().dispatch(ev)
}

/// Build the viewer screen.
pub fn view(viewer: &Shared, out: &Outbox<Msg>) -> Node {
    Node::leaf(
        Screen::new()
            // The application's word, not the toolkit's: `symbian-ui` ships no text.
            .title(crate::strings::media_photo())
            .content(Image { viewer: viewer.clone(), out: out.clone() })
            .softkeys(softkeys()),
    )
}

/// The image band: the toolkit's blit, and the pan keys.
struct Image {
    viewer: Shared,
    out: Outbox<Msg>,
}

impl Widget for Image {
    /// Constant: the size is the band, and the image's own dimensions do not change it. Panning does
    /// not resize anything either, which is why it is absent — see `ScrollList`'s digest for the same
    /// argument about a scroll offset.
    fn content_hash(&self) -> WidgetHash {
        hash_i32(0, 0x1_A6E)
    }

    fn measure(&self, c: Constraints, _t: &Theme<'_>) -> Size {
        c.constrain(Size::new(c.max_w, c.max_h))
    }

    fn draw(&self, c: &mut Canvas<'_>, rect: Rect, _theme: &Theme<'_>) {
        if let Ok(v) = self.viewer.try_borrow() {
            v.draw_image(c, rect);
        }
    }

    fn handle_key(&self, ev: KeyEvent, rect: Rect, _cx: &mut KeyCtx<'_>) -> Handled {
        let Ok(mut v) = self.viewer.try_borrow_mut() else { return Handled::Ignored };
        // The band, inset the way the drawing insets it — `Viewer::handle_key` takes the area rather
        // than the screen precisely so that panning and drawing cannot disagree about it.
        let (handled, action) = v.handle_key(ev, rect.inset_xy(2, 2));
        if let ViewerAction::Back = action {
            self.out.push(Msg::Back);
        }
        handled
    }
}
