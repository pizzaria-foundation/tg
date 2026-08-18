//! The application as a model, an update and a view.
//!
//! ```text
//!   symbian_app::entry!  /  symbian_sim::run
//!            │  keys, raw events, draw
//!            ▼
//!   ┌────────────────────────────────────────────────────────────┐
//!   │ Shell            keeps handle_raw, the driver, the worker   │
//!   └───────┬────────────────────────────────────────┬───────────┘
//!           │ keys, draw                             │ raw events
//!           ▼                                        ▼
//!   DeclarativeAppBridge<Tg>                    App::handle_raw
//!           │                                        │
//!    on_key │ update │ view                          └── the model changed:
//!           ▼                                            Msg::Touched
//!   ┌──────────────────────┬─────────────────────────────────────┐
//!   │ chats_decl::view     │ Imperative(App)                     │
//!   │ the dialog list      │ login, conversation, viewer         │
//!   └──────────────────────┴─────────────────────────────────────┘
//! ```
//!
//! # Why the whole old application is the model
//!
//! Because a migration is not a rewrite. [`App`] holds the store, the login machine, the driver and
//! four screens' worth of state, and moving all of that into an MVU model at once is the big-bang
//! this project decided against — `docs/decl-ui.md` puts it plainly: a working screen rewritten
//! declaratively is a working screen with new bugs in it.
//!
//! So the model is `Rc<RefCell<App>>` and one screen has left it. Every screen still written by hand
//! is reached through [`Imperative`], which calls the old `draw` and the old `handle_key` — the same
//! code that shipped, at the same rects. The dialog list is described declaratively and compared
//! against its predecessor pixel for pixel by `examples/chats_parity.rs`. The next screen moves the
//! same way, and when the last one has, the `RefCell` and this comment go with it.
//!
//! # The three seams, and what each one is for
//!
//! * **`Msg::Chats`** — the declarative screen's messages, wrapped. The cursor moves in `update`,
//!   the pagination request arrives from the list widget, and both go through the same door as a
//!   softkey.
//! * **`Msg::Touched`** — "the imperative side ran and the model may have moved". Pushed by the
//!   adapter after a key it consumed, and sent by the shell after a raw event. It exists because the
//!   bridge deliberately does *not* rebuild the view for a key a widget absorbed — which is right
//!   for a text field's caret and wrong for an old screen that has just switched to another screen.
//! * **The shell** — `handle_raw`, which is not the bridge's to route: the driver's completions are
//!   not keys, and the plan for this migration says the shell keeps them.
//!
//! Nothing comes out of `DeclarativeAppBridge::take_effects` yet, and there is no shell code reading
//! it, because every message so far is answered by calling a method on the old application — which
//! holds the driver. The first screen to leave the adapter with an effect of its own is the one that
//! has to add that drain, and a `Cmd` queued with nobody draining it is an effect that never happens.

use alloc::rc::Rc;
use core::cell::RefCell;

use symbian_decl_ui::app::DeclarativeApp;
use symbian_decl_ui::bridge::DeclarativeAppBridge;
use symbian_decl_ui::cmd::Cmd;
use symbian_decl_ui::keys::Softkeys;
use symbian_decl_ui::outbox::Outbox;
use symbian_decl_ui::slot::SlotTable;
use symbian_decl_ui::widgets::{Imperative, Node};

use symbian_ui::{App as _, Canvas, Handled, KeyEvent, Rect, Theme};

use crate::chats_decl;
use crate::App;

/// Everything the application knows, which for now is the application.
pub struct Model {
    /// The old app, whole. `RefCell` because a view is built from `&Model` and the screens it wraps
    /// want `&mut self` — see [`Imperative`], which is where the borrow is actually taken.
    app: Rc<RefCell<App>>,
    /// Where the widgets leave messages: the list's cursor, its request for another page, and the
    /// adapter's "something happened in there".
    out: Outbox<Msg>,
}

impl Model {
    fn new(app: App) -> Self {
        Self { app: Rc::new(RefCell::new(app)), out: Outbox::new() }
    }
}

/// What can happen to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// The dialog list said something.
    Chats(chats_decl::Msg),
    /// The imperative side ran; whatever it changed, the screen no longer describes the model.
    ///
    /// Deliberately carries nothing. It is not an event, it is an admission — the old code changed
    /// something and the only honest thing to say about it from out here is that the view is stale.
    Touched,
}

/// The application, as the bridge sees it.
pub struct Tg;

impl DeclarativeApp for Tg {
    type Model = Model;
    type Message = Msg;
    /// Navigation is still the old app's, inside its own `Screen` enum, so the bridge's stack is
    /// unused. It becomes real when the conversation leaves the adapter.
    type Screen = ();
    const TITLE: &'static str = "Telegram";

    /// The mock store, which is what the simulator and the previews want.
    ///
    /// The device does not use this: `App::login()` opens a connection as it is constructed, and
    /// which of the two an app starts from is the *host's* decision — see
    /// [`DeclarativeAppBridge::with_model`], and [`live`] below.
    fn init() -> Model {
        Model::new(App::mock())
    }

    /// The softkeys, when the screen in front declares any.
    ///
    /// One declaration, drawn by `chats_decl::view_rows` and dispatched here — [`Softkeys::map`]
    /// carries it into this app's message type without a slot going missing. Every other screen
    /// draws and routes its own bar inside the adapter, so the bar here is empty: a label declared
    /// out here would answer the key before the old screen ever saw it.
    fn keys(m: &Model) -> Softkeys<Msg> {
        let app = m.app.borrow();
        if app.on_chat_list() {
            chats_decl::softkeys(app.store.dialogs_loading).map(Msg::Chats)
        } else {
            Softkeys::new()
        }
    }

    fn on_key(m: &Model, ev: KeyEvent) -> Option<Msg> {
        // Only the dialog list is claimed at this level. `None` for everything else is what lets the
        // key reach the adapter — the bridge asks the app first, so an arm here would win.
        let app = m.app.borrow();
        if !app.on_chat_list() {
            return None;
        }
        chats_decl::on_key(app.store.dialogs_loading, ev).map(Msg::Chats)
    }

    fn outbox(m: &Model) -> Option<&Outbox<Msg>> {
        Some(&m.out)
    }

    fn update(m: &mut Model, msg: Msg) -> Cmd<()> {
        let mut app = m.app.borrow_mut();
        match msg {
            // Nothing to do: `send` has already dropped the tree, which is the whole request.
            Msg::Touched => Cmd::None,
            Msg::Chats(c) => match c {
                chats_decl::Msg::Select(i) => {
                    // Clamped against the list as it stands. The widget clamps too, and the two
                    // agree — but the message may also arrive after a page has been replaced by a
                    // shorter one, and then it is this clamp that keeps `Open` in range.
                    app.chats_selected = i.min(app.store.chats.len().saturating_sub(1));
                    Cmd::None
                }
                chats_decl::Msg::Open => {
                    let i = app.chats_selected;
                    // An empty list has nothing to open. The bar still says "Abrir", because the
                    // hand-written screen says it and that screen is what the comparison measures
                    // against; pressing it does nothing, exactly as `ChatList::activate` did.
                    if i < app.store.chats.len() {
                        app.open_chat(i);
                    }
                    Cmd::None
                }
                chats_decl::Msg::Refresh => {
                    app.refresh_dialogs();
                    Cmd::None
                }
                chats_decl::Msg::LoadMore => {
                    app.load_more_dialogs();
                    Cmd::None
                }
                // Through the bridge's flag rather than the app's, so that the one place the host
                // asks — `should_exit` — is answered by both halves. See [`Shell::should_exit`].
                chats_decl::Msg::Quit => Cmd::Exit,
            },
        }
    }

    fn view(m: &Model, slots: &mut SlotTable) -> Node {
        // Read for the length of the build only. What the adapter carries away is the `Rc`, not this
        // borrow — it takes its own, later, when a key or a frame arrives.
        let app = m.app.borrow();
        if app.on_chat_list() {
            // The screen owns its own message type; `wrapped` is the door between it and this one.
            // The queue it hands over is a handle on the app's, so the closures inside the widgets
            // keep pushing into it long after this call has returned.
            return chats_decl::view(
                // Projected here, every rebuild. A `Chat` carries its messages and their inline
                // JPEGs; a row carries seven small values. Caching the slice on the model is the
                // next move if a measurement ever asks for it — and a stale dialog list is a worse
                // bug than a bounded allocation, so it waits for the measurement.
                chats_decl::rows(&app.store),
                app.store.dialogs_loading,
                &app.store.status,
                app.chats_selected,
                &m.out.wrapped(Msg::Chats),
                slots,
            );
        }

        // Everything else, still hand-written, still what ships.
        let touched = m.out.clone();
        Node::leaf(
            Imperative::new(m.app.clone(), |app, c, _rect, theme| app.draw(c, theme)).on_key(
                move |app: &mut App, ev: KeyEvent, rect: Rect, cx| {
                    let handled = app.handle_key(ev, cx.theme, rect);
                    // A key a widget consumed does not rebuild the view — which is correct for a
                    // caret and wrong here, because the old screen may have just navigated. Saying
                    // so costs one message; not saying it once cost a blank screen on the way out
                    // of a conversation, since the chat list has no adapter to draw.
                    if handled == Handled::Consumed {
                        touched.push(Msg::Touched);
                    }
                    handled
                },
            ),
        )
    }
}

/// The application the hosts run: the bridge, plus what the bridge does not carry.
///
/// # Why there is a shell at all
///
/// [`DeclarativeAppBridge`] answers three of the four things a host asks — a key, a frame, whether
/// to close. The fourth is [`handle_raw`](symbian_ui::App::handle_raw), and this client's raw events
/// are not keys: they are socket completions, timer ticks and the image codec reporting. They belong
/// to the driver, the driver is inside the old app, and the plan for this migration says the shell
/// keeps them. So this type owns the same `Rc` the model holds, hands raw events straight to
/// `App::handle_raw`, and tells the bridge afterwards that the model has moved.
pub struct Shell {
    bridge: DeclarativeAppBridge<Tg>,
    /// The same application the model holds. Not a second one — a second handle, so the raw path
    /// does not have to go through a model it is not allowed to mutate.
    app: Rc<RefCell<App>>,
}

impl Shell {
    /// Wrap an application that has already been built.
    pub fn new(app: App) -> Self {
        let model = Model::new(app);
        let app = model.app.clone();
        Self { bridge: DeclarativeAppBridge::with_model(model), app }
    }

    /// The application inside, to read. For tests and for the shim's title handling.
    pub fn app(&self) -> core::cell::Ref<'_, App> {
        self.app.borrow()
    }

    /// Reach the application inside and change it, then tell the bridge the model has moved.
    ///
    /// The door the raw path and the tests come through, and it is one door rather than a
    /// `&mut App` because of what has to happen *after* the change: the bridge is holding a tree
    /// built from the model as it was, and nothing else would drop it. Handing out the borrow alone
    /// would work for as long as every caller remembered — and a caller that forgot would leave a
    /// screen showing the state before the reply arrived, which is indistinguishable from a reply
    /// that never came.
    pub fn with_app<R>(&mut self, f: impl FnOnce(&mut App) -> R) -> R {
        let out = f(&mut self.app.borrow_mut());
        self.bridge.send(Msg::Touched);
        out
    }
}

/// The client with a connection, for the device.
pub fn live() -> Shell {
    Shell::new(App::login())
}

/// The client with the mock store and nothing behind it, for the simulator and the previews.
pub fn mock() -> Shell {
    Shell::new(App::mock())
}

/// The login screen with no credentials, for the previews.
pub fn mock_login() -> Shell {
    Shell::new(App::mock_login())
}

impl symbian_ui::App for Shell {
    fn handle_key(&mut self, ev: KeyEvent, theme: &Theme<'_>, screen: Rect) -> Handled {
        self.bridge.handle_key(ev, theme, screen)
    }

    fn draw(&mut self, c: &mut Canvas<'_>, theme: &Theme<'_>) {
        self.bridge.draw(c, theme)
    }

    /// Straight to the old application, then a note to the bridge.
    ///
    /// The note matters. `handle_raw` is where a `getDialogs` reply lands, where the login machine
    /// becomes authorized and switches screen, where a decoded photo opens the viewer — all of it
    /// through `&mut App`, none of it through `update`. The tree the bridge is holding describes the
    /// model as it was before that, so it is dropped: `Msg::Touched` says exactly that and nothing
    /// more.
    ///
    /// Only when the event was consumed. `Ignored` is the old app saying it recognised nothing —
    /// `Outcome::None`, an image completion belonging to a decode it has already abandoned — and
    /// rebuilding the screen for those would put a view build on every keystroke the platform
    /// delivers, since a key arrives here first on its way to being translated.
    fn handle_raw(&mut self, ev: &symbian_ui::RawEvent) -> Handled {
        let handled = self.app.borrow_mut().handle_raw(ev);
        if handled == Handled::Consumed {
            self.bridge.send(Msg::Touched);
        }
        handled
    }

    /// Either half may have decided to go.
    ///
    /// The declarative screen's "Sair" becomes `Cmd::Exit` and lands on the bridge's flag; the old
    /// screens set `App::should_exit` directly, and the red key does it from anywhere. A host that
    /// asked only one of them would find an application that closes on some screens.
    fn should_exit(&self) -> bool {
        self.bridge.should_exit() || self.app.borrow().should_exit
    }

    fn title(&self) -> &str {
        Tg::TITLE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbian_gfx::Size;
    use symbian_ui::{Key, Softkey};

    const SCREEN: Size = Size::new(320, 240);

    /// A shell, a theme, and a frame already drawn — the state a phone is in by the time anyone
    /// presses anything.
    fn with_shell(f: impl FnOnce(&mut Shell, &Theme<'_>)) {
        let atlases = symbian_preview::Atlases::load();
        let mut ran = false;
        atlases.with_themes(|dark, _light| {
            let mut shell = mock();
            frame(&mut shell, dark);
            f(&mut shell, dark);
            ran = true;
        });
        assert!(ran, "the atlases did not yield a theme");
    }

    fn frame(shell: &mut Shell, theme: &Theme<'_>) {
        let mut buf = alloc::vec![0u16; (SCREEN.w * SCREEN.h) as usize];
        let mut c = Canvas::from_slice(&mut buf, SCREEN);
        shell.draw(&mut c, theme);
    }

    fn press(shell: &mut Shell, theme: &Theme<'_>, k: Key) -> Handled {
        shell.handle_key(KeyEvent::new(k), theme, Rect::from_size(SCREEN))
    }

    #[test]
    fn the_cursor_moves_through_update_and_the_list_reports_it() {
        with_shell(|shell, t| {
            assert_eq!(shell.app().chats_selected, 0);
            assert_eq!(press(shell, t, Key::Down), Handled::Consumed);
            assert_eq!(shell.app().chats_selected, 1, "the list's report never reached update");
            assert_eq!(press(shell, t, Key::Up), Handled::Consumed);
            assert_eq!(shell.app().chats_selected, 0);
        });
    }

    #[test]
    fn the_green_key_opens_the_highlighted_chat() {
        // Through the whole stack this time: `chats_decl::on_key` claims `Key::Call`, the app wraps
        // it, `update` opens the chat, and the screen behind it becomes a conversation.
        with_shell(|shell, t| {
            press(shell, t, Key::Down);
            assert_eq!(press(shell, t, Key::Call), Handled::Consumed);
            assert_eq!(shell.app().in_conversation(), Some(1));
        });
    }

    #[test]
    fn the_left_softkey_asks_for_the_list_again() {
        with_shell(|shell, t| {
            assert_eq!(press(shell, t, Key::Softkey(Softkey::Left)), Handled::Consumed);
            // No connection behind the mock, and saying so is the whole of what "Atualizar" can do
            // here — what matters is that the message arrived at `refresh_dialogs` at all.
            assert_eq!(shell.app().store.status, "sem conexao");
        });
    }

    #[test]
    fn leaving_a_conversation_shows_the_list_rather_than_nothing() {
        // The regression the adapter's `Msg::Touched` exists for. Backing out of a conversation is a
        // key the *widget* consumed, and the bridge does not rebuild the view for one of those — so
        // without the message the tree would still be the adapter, and the adapter would draw the
        // chat list arm of the old `paint`, which is now empty. A blank screen.
        with_shell(|shell, t| {
            press(shell, t, Key::Down);
            press(shell, t, Key::Select);
            assert_eq!(shell.app().in_conversation(), Some(1));
            frame(shell, t);

            press(shell, t, Key::Softkey(Softkey::Right));
            assert!(shell.app().on_chat_list());

            let mut buf = alloc::vec![0u16; (SCREEN.w * SCREEN.h) as usize];
            let mut c = Canvas::from_slice(&mut buf, SCREEN);
            shell.draw(&mut c, t);
            assert!(buf.iter().any(|&px| px != 0), "the dialog list drew nothing at all");
        });
    }

    #[test]
    fn a_key_nobody_wanted_changes_nothing() {
        with_shell(|shell, t| {
            assert_eq!(press(shell, t, Key::Char('q')), Handled::Ignored);
            assert_eq!(shell.app().chats_selected, 0);
            assert!(!shell.should_exit());
        });
    }

    #[test]
    fn sair_on_the_list_closes_the_application() {
        // Through the bridge's flag, which is why `Shell::should_exit` asks both halves.
        with_shell(|shell, t| {
            assert!(!shell.should_exit());
            press(shell, t, Key::Softkey(Softkey::Right));
            assert!(shell.should_exit());
        });
    }

    #[test]
    fn a_change_made_behind_the_bridge_shows_up_on_the_next_frame() {
        // `with_app` is the door the raw path uses: a reply lands in the store without `update` ever
        // running, so the tree the bridge is holding describes the model as it was. The proof that it
        // is dropped is in the pixels — the status is drawn as the title bar's detail, so a stale
        // tree would paint the old string and this frame would be identical to the last one.
        with_shell(|shell, t| {
            let shot = |shell: &mut Shell| {
                let mut buf = alloc::vec![0u16; (SCREEN.w * SCREEN.h) as usize];
                let mut c = Canvas::from_slice(&mut buf, SCREEN);
                shell.draw(&mut c, t);
                buf
            };
            let before = shot(shell);
            shell.with_app(|a| a.store.status = alloc::string::String::from("recebido"));
            let after = shot(shell);
            assert_ne!(before, after, "the frame after the change was built from the model before it");
        });
    }
}
