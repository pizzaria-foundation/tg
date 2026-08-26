//! The conversation, described around a transcript that is still drawn by hand.
//!
//! ```text
//!   ┌──────────────────────────────────────────┐  TitleBar — the chat's name, and
//!   ├──────────────────────────────────────────┤  whatever the screen last reported
//!   │  ┌────────────────────┐                  │
//!   │  │ oi, tudo bem?      │             ┌────┤  Transcript — a leaf. Bubbles, link
//!   │  └────────────────────┘             │ ▓▓ │  runs, media labels, wrapped text
//!   │              ┌──────────────────┐   │    │  and a scrollbar: custom drawing,
//!   │              │ tudo, e você?    │   └────┤  which is the case `docs/decl-ui.md`
//!   │              └──────────────────┘        │  says to leave alone.
//!   ├──────────────────────────────────────────┤
//!   │ Mensagem…                                │  Composer — the `footer` band, which
//!   ├──────────────────────────────────────────┤  exists for exactly this shape
//!   │  Atualizar      Enviar        Voltar     │  SoftkeyBar
//!   └──────────────────────────────────────────┘
//! ```
//!
//! # What actually changed, and what deliberately did not
//!
//! The *chrome* is declarative: the title bar, the two bands and the softkey bar are `Screen`'s, and
//! the band arithmetic is `Frame::split` plus the footer's measured height — the same two numbers
//! `Conversation::bands` produced by hand, now produced once.
//!
//! Everything inside the bands is the code that shipped. `Conversation::draw_transcript` and
//! `draw_composer` are the same functions the hand-written screen calls; the widgets below place
//! them and nothing more. A wrapped bubble with a link running across two lines is not something to
//! re-express as a tree of rectangles — it is a paragraph layout, and the version that works is the
//! one that has been on a phone.
//!
//! # Why the transcript answers the composer's keys
//!
//! `Conversation::handle_key_in` already routes by focus: with the composer focused it types, with
//! the transcript focused it walks messages and links, and both are one function because the
//! *transitions* between them live in the middle of it — Up out of the composer, Down off the end of
//! the transcript. Splitting that into two widgets would mean each one knowing when to hand over,
//! which is a rule in two places instead of the one it is today.
//!
//! So the transcript leaf takes every key the screen does not claim and hands it straight to that
//! function, and the composer leaf draws only. The focus flag lives where it always did.
//!
//! # The state is the application's, because the transcript needs the messages
//!
//! Both widgets hold `Rc<RefCell<App>>`. A transcript cannot be drawn from a projection — it needs
//! the message text, the media, the delivery ticks — and a `Chat` carries its whole window, inline
//! JPEGs and all, so copying one per frame is exactly what the dialog list's migration removed. The
//! application already lives behind that `Rc` for the adapter's sake; when the store becomes a model
//! of its own, this is where it will show.

use alloc::rc::Rc;
use alloc::string::String;
use core::cell::RefCell;

use symbian_decl_ui::outbox::Outbox;
use symbian_decl_ui::widget::{hash_i32, KeyCtx, Widget, WidgetHash};
use symbian_decl_ui::widgets::{Screen, TitleBar};
use symbian_decl_ui::{Constraints, Handled, Key, KeyEvent, Node, Softkeys};
use symbian_ui::{Canvas, Rect, Size, Theme};

use crate::conv::{ConvAction, Focus};
use crate::App;

/// What the conversation can ask the application to do.
///
/// One variant per [`ConvAction`] that means something, which is every one of them: this screen's
/// actions were already messages in all but name — a value returned from `handle_key` for the
/// application to act on after the borrow ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// Back to the dialog list.
    Back,
    /// Send what is in the composer. The bar's middle key, when the composer has focus.
    Send,
    /// The action key with the transcript focused: open the link the cursor is on, or the
    /// highlighted message's media. *Which* of those is the conversation's to say — see
    /// `Conversation::activate` — and it is resolved in `update` rather than here, because the bar
    /// owns this key and a bar cannot ask where a cursor is.
    Activate,
    /// Re-fetch the newest page: there is no push behind this screen.
    Refresh,
    /// The transcript reached the top; ask for the page above.
    LoadMore,
    /// Select on a message with media.
    OpenMedia(usize),
    /// Select on a focused link, with the URL as written in the message.
    OpenLink(String),
    /// Copy what the cursor is on — a focused link, or the whole message.
    Copy(String),
    /// Text already taken out of the composer by the hand-written routing. See [`message_for`].
    SendTaken(String),
    /// Something the *description* of this screen depends on moved while a widget was answering a
    /// key. See [`ViewState`].
    ViewStale,
}

/// Build the conversation screen.
pub fn view(app: &Rc<RefCell<App>>, out: &Outbox<Msg>) -> Node {
    let borrowed = app.borrow();
    let Some(conv) = borrowed.conversation() else {
        // No conversation in front. Cannot happen through `mvu`, which checks first, and a blank
        // screen beats a panic on a phone whose entire failure report is a dialog with a number.
        return Node::leaf(Transcript { app: app.clone(), out: out.clone() });
    };
    let name = borrowed
        .store
        .chats
        .get(conv.chat)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    let subtitle = borrowed
        .store
        .chats
        .get(conv.chat)
        .and_then(|c| conv.subtitle(c))
        .map(String::from);
    let keys = softkeys(conv.composer.is_empty());
    drop(borrowed);

    let mut bar = TitleBar::new(name);
    if let Some(sub) = subtitle {
        bar = bar.detail(sub);
    }
    Node::leaf(
        Screen::new()
            .title_bar(bar)
            .content(Transcript { app: app.clone(), out: out.clone() })
            .footer(Composer { app: app.clone() })
            .softkeys(keys),
    )
}

/// The softkey bar: refresh, send when there is something to send, back.
///
/// One declaration, drawn by [`view`] and dispatched by [`on_key`]. The middle slot appears with the
/// first character typed and goes away with the last one deleted, which is what the hand-written
/// screen does — and is the reason `Screen`'s labels come from the same value the dispatch reads.
pub fn softkeys(composer_empty: bool) -> Softkeys<Msg> {
    let bar = Softkeys::new().options(crate::strings::refresh(), Msg::Refresh).back(symbian_ui::strings::back(), Msg::Back);
    if composer_empty {
        bar
    } else {
        bar.action(crate::strings::send(), Msg::Send)
    }
}

/// What a key means to this screen, before the transcript sees it.
///
/// # The action key belongs to whichever half has focus
///
/// With the composer focused it sends; with the transcript focused it opens the media on the
/// highlighted message, or the link the cursor is on. The bar cannot say that — it has one middle
/// slot — so the action is claimed here only when the composer has the keyboard, and otherwise falls
/// through to the transcript, which knows what the cursor is on.
///
/// That leaves the bar reading Enviar while Select opens a photo, whenever there is text in the
/// composer and focus is in the transcript. It is the hand-written screen's behaviour exactly, and it
/// is the label-lies-about-the-key shape this crate's `keys` module exists to prevent — recorded here
/// rather than fixed, because fixing it means changing what the bar draws and the comparison this
/// screen must pass is against the screen that draws it.
pub fn on_key(focus: Focus, composer_empty: bool, ev: KeyEvent) -> Option<Msg> {
    let action = matches!(
        ev.key,
        Key::Select | Key::Enter | Key::Call | Key::Softkey(symbian_ui::Softkey::Middle)
    );
    if action {
        // Claimed either way, and that is not a choice: `Screen` offers a key to its bar before its
        // content, so a labelled action can never reach the transcript. Leaving it unclaimed here
        // would make Select do *nothing* whenever there is text in the composer.
        return Some(match focus {
            Focus::Transcript => Msg::Activate,
            Focus::Composer => Msg::Send,
        });
    }
    softkeys(composer_empty).dispatch(ev)
}

/// What [`view`] reads out of the conversation besides the two bands.
///
/// # Why this has to exist
///
/// The bridge does not rebuild the view for a key a widget consumed, and it is right not to: a caret
/// moving is not a change to the *description* of a screen. Except when it is. Type the first
/// character into this composer and the softkey bar gains Enviar — a label that lives in the tree,
/// built the last time `view` ran. Without saying so, the bar appears on the next keypress that
/// happens to invalidate for another reason, or never.
///
/// It was not the parity harness that found that: a comparison renders one state and builds a fresh
/// tree for it, so a stale tree is exactly what it cannot see. It was `examples/preview.rs`, which
/// drives keys and *then* draws — the same order the device does.
///
/// So this is the list of everything `view` looks at, compared before and after a key. Anything added
/// to the view has to be added here, or it goes stale the same way.
#[derive(Clone, PartialEq, Eq)]
struct ViewState {
    /// The title bar's detail line, which the routing writes to on half a dozen paths.
    note: Option<String>,
    /// Whether the bar offers Enviar.
    composer_empty: bool,
}

impl ViewState {
    fn of(conv: &crate::conv::Conversation) -> Self {
        Self { note: conv.note.clone(), composer_empty: conv.composer.is_empty() }
    }
}

/// The transcript band: the hand-written drawing, and every key the screen did not claim.
struct Transcript {
    app: Rc<RefCell<App>>,
    out: Outbox<Msg>,
}

impl Widget for Transcript {
    /// Constant, because the size is the band it is given and nothing else — the same argument
    /// `Imperative` makes, and the cache keys on the offer as well as on this digest.
    fn content_hash(&self) -> WidgetHash {
        hash_i32(0, 0x7A_5C)
    }

    fn measure(&self, c: Constraints, _t: &Theme<'_>) -> Size {
        c.constrain(Size::new(c.max_w, c.max_h))
    }

    fn draw(&self, c: &mut Canvas<'_>, rect: Rect, theme: &Theme<'_>) {
        let Ok(mut app) = self.app.try_borrow_mut() else { return };
        if let Some((conv, chat)) = app.conversation_and_chat() {
            conv.draw_transcript(c, rect, chat, theme);
        }
    }

    fn handle_key(&self, ev: KeyEvent, rect: Rect, cx: &mut KeyCtx<'_>) -> Handled {
        let Ok(mut app) = self.app.try_borrow_mut() else { return Handled::Ignored };
        let Some((conv, chat)) = app.conversation_and_chat() else { return Handled::Ignored };
        let before = ViewState::of(conv);
        // The band this widget was placed in *is* the transcript viewport, so the routing gets the
        // number it needs instead of deriving it from the screen a second way.
        let (handled, action) = conv.handle_key_in(ev, chat, cx.theme, rect);
        let stale = ViewState::of(conv) != before;
        if let Some(msg) = message_for(action) {
            self.out.push(msg);
        } else if stale {
            // No action, but the screen's own description moved: a note was written, or the first
            // character arrived in the composer and the bar has an Enviar to draw now.
            self.out.push(Msg::ViewStale);
        }
        handled
    }
}

/// The composer band: drawn here, typed into through the transcript.
///
/// Keys are not offered to it — `Screen` asks the footer first, and answering here would take a
/// keystroke away from the one function that knows whether the composer has focus. See the module
/// header.
struct Composer {
    app: Rc<RefCell<App>>,
}

impl Widget for Composer {
    fn content_hash(&self) -> WidgetHash {
        hash_i32(0, 0xC0_3B)
    }

    /// One line of the body font plus the padding either side — the number
    /// `Conversation::composer_h` produces, which is what makes the two bands come out where the
    /// hand-written screen puts them.
    fn measure(&self, c: Constraints, theme: &Theme<'_>) -> Size {
        c.constrain(Size::new(c.max_w, theme.fonts.body.line_height() + 8))
    }

    fn draw(&self, c: &mut Canvas<'_>, rect: Rect, theme: &Theme<'_>) {
        let Ok(app) = self.app.try_borrow() else { return };
        if let Some(conv) = app.conversation() {
            conv.draw_composer(c, rect, theme);
        }
    }
}

/// The message an action means, or `None` for the ones that are not events.
fn message_for(action: ConvAction) -> Option<Msg> {
    match action {
        ConvAction::Back => Some(Msg::Back),
        // Unreachable from the declarative screen: the action key never gets this far, because
        // `on_key` claims it above. Mapped rather than ignored so that a text already taken out of
        // the field cannot be dropped on the floor if some other path ever produces one.
        ConvAction::Send(text) => Some(Msg::SendTaken(text)),
        ConvAction::Refresh => Some(Msg::Refresh),
        ConvAction::LoadMore => Some(Msg::LoadMore),
        ConvAction::OpenMedia(i) => Some(Msg::OpenMedia(i)),
        ConvAction::OpenLink(url) => Some(Msg::OpenLink(url)),
        ConvAction::Copy(text) => Some(Msg::Copy(text)),
        ConvAction::None => None,
    }
}

#[cfg(test)]
mod parity {
    //! The conversation, drawn twice, compared pixel for pixel.
    //!
    //! # Why this one is a test and not an example
    //!
    //! The other two comparisons are `examples/*_parity.rs`, which is the better shape: a human can
    //! run them, and `test = true` makes `cargo test` run them too. This screen's state cannot be
    //! built from outside the crate — `App::screen` is private, the `Screen` enum is private, and a
    //! conversation's link cursor is reached by walking it with keys — so an example would have needed
    //! a public constructor per scene. Inventing API so a comparison can exist is how a test starts
    //! deciding what the code looks like, so the comparison moved inside instead.
    //!
    //! It runs under `cargo test` like the others and writes the same pictures to `parity-out/`.

    use super::*;
    use alloc::string::ToString;
    use alloc::vec::Vec;
    use symbian_decl_ui::layout;
    use symbian_decl_ui::slot::SlotTable;
    use symbian_decl_ui::UiCache;
    use symbian_gfx::E72_SCREEN;
    use symbian_preview::{Atlases, Parity};
    use symbian_ui::{Key, KeyEvent};

    use crate::conv::Conversation;
    use crate::model::{Chat, Delivery, Media, Message, Store};

    /// One scene: the chat, and what the user has done to the screen.
    struct Scene {
        name: &'static str,
        chat: Chat,
        /// Which message the cursor is on, counted from the newest backwards — `0` is the newest.
        from_newest: usize,
        focus: Focus,
        /// How many times to press Down from the parked position, which is how a link cursor is
        /// reached: the walk is message, then its links, then the next message.
        walk_down: usize,
        typed: &'static str,
        note: Option<&'static str>,
    }

    fn msg(text: &str, outgoing: bool) -> Message {
        Message {
            id: 1,
            text: text.to_string(),
            outgoing,
            time: "12:00".to_string(),
            state: Delivery::Read,
            media: None,
        }
    }

    fn media_msg(text: &str, media: Media) -> Message {
        Message { media: Some(media), ..msg(text, false) }
    }

    fn chat_of(messages: Vec<Message>) -> Chat {
        Chat { name: "Ana Paula".to_string(), messages, ..Chat::default() }
    }

    fn scenes() -> Vec<Scene> {
        let plain = || {
            chat_of(alloc::vec![
                msg("oi, tudo bem?", false),
                msg("tudo! e você?", true),
                msg("por aqui também", false),
            ])
        };
        let long = || {
            chat_of(alloc::vec![
                msg("olha só", false),
                msg(
                    "esse texto é longo o suficiente para quebrar em várias linhas dentro do balão \
                     e ainda sobrar mais uma, que é exatamente o caso que a marca de hora inline \
                     tem de resolver",
                    false,
                ),
                msg("pois é", true),
            ])
        };
        let linked = || {
            chat_of(alloc::vec![
                msg("veja isto", false),
                msg("https://exemplo.com/uma/pagina?x=1 e mais texto depois", false),
            ])
        };
        let with_media = || {
            chat_of(alloc::vec![
                msg("mandei a foto", false),
                media_msg(
                    "",
                    Media::Photo {
                        id: 1,
                        access_hash: 0,
                        file_reference: Vec::new(),
                        dc_id: 2,
                        thumb_size: "m".to_string(),
                        size: 47_104,
                        preview: None,
                    },
                ),
            ])
        };

        alloc::vec![
            Scene { name: "conv-composer", chat: plain(), from_newest: 0, focus: Focus::Composer, walk_down: 0, typed: "", note: None },
            Scene { name: "conv-composer-typed", chat: plain(), from_newest: 0, focus: Focus::Composer, walk_down: 0, typed: "vou testar", note: None },
            Scene { name: "conv-transcript", chat: plain(), from_newest: 0, focus: Focus::Transcript, walk_down: 0, typed: "", note: None },
            Scene { name: "conv-transcript-older", chat: plain(), from_newest: 2, focus: Focus::Transcript, walk_down: 0, typed: "", note: None },
            // Enviar on the bar while the cursor is in the transcript: the label says one thing and
            // the key does another, which is the hand-written screen's behaviour and a scene rather
            // than a footnote.
            Scene { name: "conv-transcript-with-text", chat: plain(), from_newest: 0, focus: Focus::Transcript, walk_down: 0, typed: "escrito", note: None },
            Scene { name: "conv-wrapped", chat: long(), from_newest: 1, focus: Focus::Transcript, walk_down: 0, typed: "", note: None },
            Scene { name: "conv-link", chat: linked(), from_newest: 0, focus: Focus::Transcript, walk_down: 0, typed: "", note: None },
            // One Down off the newest message steps onto its first link, which is the only way to
            // reach the link cursor: it is not model state, it is a position in the laid-out text.
            Scene { name: "conv-link-focused", chat: linked(), from_newest: 0, focus: Focus::Transcript, walk_down: 1, typed: "", note: None },
            Scene { name: "conv-media", chat: with_media(), from_newest: 0, focus: Focus::Transcript, walk_down: 0, typed: "", note: None },
            Scene { name: "conv-note", chat: plain(), from_newest: 0, focus: Focus::Composer, walk_down: 0, typed: "", note: Some("atualizando…") },
            Scene { name: "conv-empty", chat: chat_of(Vec::new()), from_newest: 0, focus: Focus::Composer, walk_down: 0, typed: "", note: None },
        ]
    }

    /// The store a scene renders against: one chat, the one in the scene.
    fn store_of(scene: &Scene) -> Store {
        Store { chats: alloc::vec![scene.chat.clone()], ..Store::default() }
    }

    /// A conversation in the scene's state, laid out for the real screen.
    ///
    /// Built the same way on both sides, from the same recipe, because `Conversation` is not `Clone`
    /// — and a scene whose two sides were arranged separately would be comparing two states.
    fn conversation_for(scene: &Scene, chat: &Chat, theme: &Theme<'_>) -> Conversation {
        let mut conv = Conversation::new(0);
        // The first layout parks the cursor on the newest message, which is what a chat client does
        // on open. Everything below is measured from there.
        let screen = Rect::from_size(E72_SCREEN);
        conv.handle_key(KeyEvent::new(Key::Char('\0')), chat, theme, screen);
        conv.focus = scene.focus;
        if scene.focus == Focus::Transcript {
            for _ in 0..scene.from_newest {
                conv.handle_key(KeyEvent::new(Key::Left), chat, theme, screen);
            }
            for _ in 0..scene.walk_down {
                conv.handle_key(KeyEvent::new(Key::Down), chat, theme, screen);
            }
        }
        for ch in scene.typed.chars() {
            conv.composer.insert(ch);
        }
        conv.note = scene.note.map(|n| n.to_string());
        conv
    }

    /// The shipping screen.
    fn render_by_hand(c: &mut symbian_ui::Canvas<'_>, scene: &Scene, theme: &Theme<'_>) {
        let store = store_of(scene);
        let chat = &store.chats[0];
        let mut conv = conversation_for(scene, chat, theme);
        conv.draw(c, chat, theme);
    }

    /// The declarative screen, through the real layout pass and the application it runs in.
    fn render_declared(c: &mut symbian_ui::Canvas<'_>, scene: &Scene, theme: &Theme<'_>) {
        let store = store_of(scene);
        let conv = conversation_for(scene, &store.chats[0], theme);
        let app = Rc::new(RefCell::new(App::in_conversation_for_test(store, conv)));
        let out: Outbox<Msg> = Outbox::new();
        let mut slots = SlotTable::new();
        let mut cache = UiCache::new();
        // Two frames, and the second is the one compared: the first fills the measure cache, which is
        // the steady state a device is always in by the time anyone looks at the screen.
        for _ in 0..2 {
            slots.begin_frame();
            let tree = view(&app, &out);
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
        // The light palette over the two scenes with the most colour in them: a bubble on each side,
        // a focused link, and a selected row. The arithmetic is the same in both palettes, so what a
        // second full pass would prove is that `Ink` resolves.
        for scene in scenes() {
            let name: &'static str = match scene.name {
                "conv-link-focused" => "conv-link-focused-light",
                "conv-media" => "conv-media-light",
                _ => continue,
            };
            p.check(
                name,
                light,
                |c| render_by_hand(c, &scene, light),
                |c| render_declared(c, &scene, light),
            );
        }
    }

    #[test]
    fn the_declared_conversation_is_the_hand_written_one() {
        let atlases = Atlases::load();
        let mut p = Parity::new("parity-out").keep_matching(true);
        atlases.with_themes(|dark, light| {
            run(&mut p, dark, light);
        });
        assert_eq!(p.checked(), 13, "a scene stopped being compared");
        p.finish();
    }

    #[test]
    fn every_scene_renders_something_different() {
        // Eleven comparisons of one state would pass and prove one thing. This renders the shipping
        // screen alone and asserts that each scene's inputs actually reach the pixels.
        let atlases = Atlases::load();
        atlases.with_themes(|dark, _light| {
            let mut seen: Vec<(&str, Vec<u16>)> = Vec::new();
            for scene in scenes() {
                let mut sheet = symbian_preview::Sheet::new(E72_SCREEN);
                render_by_hand(&mut sheet.canvas(), &scene, dark);
                let px = sheet.pixels().to_vec();
                for (name, other) in &seen {
                    assert_ne!(&px, other, "{} drew the same pixels as {name}", scene.name);
                }
                seen.push((scene.name, px));
            }
        });
    }
}
