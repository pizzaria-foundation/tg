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
use symbian_decl_ui::widgets::Node;

use symbian_ui::{Canvas, Handled, KeyEvent, Rect, Theme};

use crate::{chats_decl, conv_decl, login_decl, menu_decl, viewer_decl};
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
    /// A login screen said something.
    Login(login_decl::Msg),
    /// The conversation said something.
    Conv(conv_decl::Msg),
    /// The photo viewer said something.
    Viewer(viewer_decl::Msg),
    /// The Options menu said something.
    Menu(menu_decl::Msg),
    /// Leave the application. The red key, from any screen.
    Exit,
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
        if app.on_menu() {
            menu_decl::softkeys().map(Msg::Menu)
        } else if app.on_chat_list() {
            chats_decl::softkeys(app.store.dialogs_loading).map(Msg::Chats)
        } else if app.on_login() {
            login_decl::softkeys(&login_decl::State::of(app.login_state())).map(Msg::Login)
        } else if let Some(conv) = app.conversation() {
            conv_decl::softkeys(conv.composer.is_empty()).map(Msg::Conv)
        } else if app.viewer().is_some() {
            viewer_decl::softkeys().map(Msg::Viewer)
        } else {
            Softkeys::new()
        }
    }

    fn on_key(m: &Model, ev: KeyEvent) -> Option<Msg> {
        // The red End key closes the application, from any screen and with any text half-typed.
        //
        // First, before anything else can want it — which is exactly where the hand-written app put
        // it, and for the reason its comment gives: the toolkit's own path only fires when every
        // widget below has returned `Ignored`, and a composer consumes whatever it is given. It has
        // to be here rather than on a bar because it is not a softkey, and because a screen whose
        // back slot means something else — the login code screen's Voltar — would otherwise answer
        // it and the phone would feel stuck.
        if ev.key == symbian_ui::Key::End {
            return Some(Msg::Exit);
        }
        // Only the dialog list is claimed at this level. `None` for everything else is what lets the
        // key reach the adapter — the bridge asks the app first, so an arm here would win.
        let app = m.app.borrow();
        if app.on_menu() {
            return menu_decl::on_key(ev).map(Msg::Menu);
        }
        if app.on_chat_list() {
            return chats_decl::on_key(app.store.dialogs_loading, ev).map(Msg::Chats);
        }
        if app.on_login() {
            return login_decl::on_key(&login_decl::State::of(app.login_state()), ev).map(Msg::Login);
        }
        if let Some(conv) = app.conversation() {
            return conv_decl::on_key(conv.focus, conv.composer.is_empty(), ev).map(Msg::Conv);
        }
        if app.viewer().is_some() {
            return viewer_decl::on_key(ev).map(Msg::Viewer);
        }
        None
    }

    fn outbox(m: &Model) -> Option<&Outbox<Msg>> {
        Some(&m.out)
    }

    fn update(m: &mut Model, msg: Msg) -> Cmd<()> {
        let mut app = m.app.borrow_mut();
        match msg {
            // Nothing to do: `send` has already dropped the tree, which is the whole request.
            Msg::Touched => Cmd::None,
            Msg::Exit => {
                symbian::log!("[act] end key: exit");
                Cmd::Exit
            }
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
                    // An empty list has nothing to open. The bar still says Abrir, because the
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
                chats_decl::Msg::Options => {
                    app.open_menu();
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
            Msg::Menu(x) => match x {
                menu_decl::Msg::Select(i) => {
                    app.menu_selected = i.min(menu_decl::entries().len().saturating_sub(1));
                    Cmd::None
                }
                menu_decl::Msg::Run => {
                    if app.menu_run() {
                        app.close_menu();
                    }
                    Cmd::None
                }
                menu_decl::Msg::Back => {
                    app.close_menu();
                    Cmd::None
                }
            },
            Msg::Login(l) => match l {
                // What to submit is the login machine's to decide — this key only says that the
                // middle softkey was pressed.
                login_decl::Msg::Submit => {
                    app.login_submit();
                    Cmd::None
                }
                login_decl::Msg::BackToPhone => {
                    app.login_back_to_phone();
                    Cmd::None
                }
                login_decl::Msg::ToggleMask => {
                    app.login_toggle_mask();
                    Cmd::None
                }
                // Back on the phone screen means leaving the application, which is what the
                // hand-written screen did with an unlabelled key. See `login_decl::on_key`.
                login_decl::Msg::Quit => Cmd::Exit,
            },
            Msg::Conv(c) => {
                use conv_decl::Msg as C;
                match c {
                    // Whatever the cursor is on: the conversation decides, the application acts.
                    C::Activate => app.conversation_activate(),
                    C::Send => app.conversation_send(),
                    C::SendTaken(text) => {
                        app.conversation_action(crate::conv::ConvAction::Send(text))
                    }
                    C::Back => app.conversation_action(crate::conv::ConvAction::Back),
                    C::Refresh => app.conversation_action(crate::conv::ConvAction::Refresh),
                    C::LoadMore => app.conversation_action(crate::conv::ConvAction::LoadMore),
                    C::OpenMedia(i) => {
                        app.conversation_action(crate::conv::ConvAction::OpenMedia(i))
                    }
                    C::OpenLink(url) => {
                        app.conversation_action(crate::conv::ConvAction::OpenLink(url))
                    }
                    C::Copy(text) => app.conversation_action(crate::conv::ConvAction::Copy(text)),
                    // Nothing to do but exist: `send` has already dropped the tree, which is the
                    // whole of what this message asks for. See `conv_decl::ViewState`.
                    C::ViewStale => {}
                }
                Cmd::None
            }
            Msg::Viewer(viewer_decl::Msg::Back) => {
                app.viewer_back();
                Cmd::None
            }
        }
    }

    fn view(m: &Model, slots: &mut SlotTable) -> Node {
        // Read for the length of the build only. What the adapter carries away is the `Rc`, not this
        // borrow — it takes its own, later, when a key or a frame arrives.
        let app = m.app.borrow();
        if app.on_menu() {
            return menu_decl::view(
                app.menu_selected,
                app.debug_log,
                &m.out.wrapped(Msg::Menu),
                slots,
            );
        }
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

        if app.on_login() {
            // The field's buffer belongs to the login machine, which is where the application can
            // still read it when the middle softkey says to submit — see `login::Field`. A waiting
            // screen has no field, and `login_decl` does not draw one, so a throwaway buffer is
            // honest here: nothing types into it and nothing reads it.
            let state = login_decl::State::of(app.login_state());
            let field = app
                .login_state()
                .field()
                .unwrap_or_else(|| crate::login::shared(symbian_ui::TextField::new()));
            return login_decl::view(&state, &field, slots);
        }

        if app.on_conversation() {
            // The transcript and the composer are leaves over this same `Rc` — a transcript cannot be
            // drawn from a copy of a chat, and a chat carries its whole message window.
            drop(app);
            return conv_decl::view(&m.app, &m.out.wrapped(Msg::Conv));
        }

        if let Some(viewer) = app.viewer() {
            let viewer = viewer.clone();
            drop(app);
            return viewer_decl::view(&viewer, &m.out.wrapped(Msg::Viewer));
        }

        // Nothing left behind the adapter.
        //
        // Every screen this application has is above, and the four of them are the whole of its
        // `Screen` enum — so this is unreachable, and it is a blank frame rather than an
        // `unreachable!()` because a panic on a phone whose entire failure report is a dialog with a
        // number in it is not worth being right about. `widgets::Imperative` was what made the
        // migration possible one screen at a time and is no longer part of this app; the `RefCell`
        // around the model stays until the store itself becomes one.
        Node::leaf(symbian_decl_ui::widgets::Spacer::new())
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
        Self {
            // The clipboard is the bridge's to hold, and giving it one is not optional: the
            // declarative field pastes through `KeyCtx`, and the hand-written login screen reached
            // for `symbian_app::SystemClipboard` directly. Without this line the login field would be
            // the one field on the phone that cannot paste — silently, since paste into an empty
            // clipboard looks exactly the same.
            bridge: DeclarativeAppBridge::with_model(model)
                .with_clipboard(symbian_app::SystemClipboard),
            app,
        }
    }

    /// Use a different clipboard than the platform's.
    ///
    /// The device wants `symbian_app::SystemClipboard` and gets it from [`Self::new`]. The *simulator*
    /// does not have one — the host's clipboard is not reachable through the shim — so a
    /// `MemClipboard` there makes copy and paste work between two fields on the same screen, which is
    /// enough to exercise every line of the editing path without a handset.
    pub fn with_clipboard(mut self, clip: impl symbian_ui::Clipboard + 'static) -> Self {
        self.bridge.set_clipboard(clip);
        self
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
    /// The declarative screen's Sair becomes `Cmd::Exit` and lands on the bridge's flag; the old
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
    use alloc::string::ToString;
    // The trait, for `draw`/`handle_key`/`should_exit` on the shell — which is what a host calls.
    use symbian_ui::{App as _, Key, Softkey};

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
    fn the_left_softkey_opens_the_options_menu() {
        with_shell(|shell, t| {
            assert_eq!(press(shell, t, Key::Softkey(Softkey::Left)), Handled::Consumed);
            assert!(shell.app().on_menu(), "the left key is Opções now, not Atualizar");
            // Its first entry is what the key used to do, and the cursor starts on it — so the
            // habit of pressing left and then the action key still refreshes the list.
            assert_eq!(shell.app().menu_selected, 0);
            assert_eq!(press(shell, t, Key::Softkey(Softkey::Middle)), Handled::Consumed);
            // No connection behind the mock, and saying so is the whole of what a refresh can do
            // here — what matters is that the message arrived at `refresh_dialogs` at all.
            assert_eq!(shell.app().store.status, crate::strings::no_connection());
            assert!(!shell.app().on_menu(), "and a refresh closes the menu over its own answer");
        });
    }

    /// The log switch is the reason the menu exists, so it is worth a test of its own: it flips, it
    /// says so in its label, and it leaves the menu open to be read.
    #[test]
    fn the_log_entry_toggles_and_keeps_the_menu_open() {
        with_shell(|shell, t| {
            press(shell, t, Key::Softkey(Softkey::Left));
            press(shell, t, Key::Down);
            let was = shell.app().debug_log;
            let before = crate::menu_decl::label(crate::menu_decl::Action::DebugLog, was);

            press(shell, t, Key::Softkey(Softkey::Middle));

            let now = shell.app().debug_log;
            assert_eq!(now, !was);
            assert_ne!(crate::menu_decl::label(crate::menu_decl::Action::DebugLog, now), before);
            assert!(shell.app().on_menu(), "the label it just changed is on this screen");
        });
    }

    /// Voltar changes nothing, which is the whole of what a menu's back key promises.
    #[test]
    fn leaving_the_menu_changes_nothing() {
        with_shell(|shell, t| {
            let status = shell.app().store.status.clone();
            press(shell, t, Key::Softkey(Softkey::Left));
            let was = shell.app().debug_log;
            press(shell, t, Key::Softkey(Softkey::Right));
            assert!(!shell.app().on_menu());
            assert_eq!(shell.app().debug_log, was);
            assert_eq!(shell.app().store.status, status, "and nothing was asked of the server");
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

    // ---- the login screens ------------------------------------------------------------------------

    /// A shell on the login screen, with a frame drawn.
    fn with_login(f: impl FnOnce(&mut Shell, &Theme<'_>)) {
        let atlases = symbian_preview::Atlases::load();
        let mut ran = false;
        atlases.with_themes(|dark, _light| {
            let mut shell = mock_login();
            // `Login::new` starts on the waiting screen, because the network is not ready on launch
            // and a number field that cannot be used reads as a broken screen. The test wants the
            // phone screen, which is what `show_phone` is for.
            shell.with_app(|a| a.show_phone_for_test());
            frame(&mut shell, dark);
            f(&mut shell, dark);
            ran = true;
        });
        assert!(ran, "the atlases did not yield a theme");
    }

    #[test]
    fn typing_reaches_the_phone_field_and_a_letter_does_not() {
        // The first screen whose keys go all the way through the tree: the softkeys are the app's and
        // everything else belongs to the field. The digits filter lives in the buffer, so it holds
        // for a paste as well as for a keystroke.
        with_login(|shell, t| {
            for ch in "1a2".chars() {
                press(shell, t, Key::Char(ch));
            }
            let field = shell.app().login_state().field().expect("the phone screen has a field");
            assert_eq!(field.borrow().text(), "12", "a letter reached a digits-only field");
        });
    }

    #[test]
    fn the_middle_softkey_submits_what_was_typed() {
        // And the application reads it out of the buffer it holds, in `update`, where there is no
        // widget to ask — which is the whole reason the field is an `Rc` and not a slot.
        with_login(|shell, t| {
            for ch in "11999990000".chars() {
                press(shell, t, Key::Char(ch));
            }
            press(shell, t, Key::Select);
            // Nothing behind the mock, so the machine parks on its waiting screen — which is what
            // says the number was submitted rather than swallowed.
            assert!(shell.app().login_state().is_waiting(), "the number was not submitted");
        });
    }

    #[test]
    fn a_key_the_login_screen_does_not_want_is_left_alone() {
        with_login(|shell, t| {
            assert_eq!(press(shell, t, Key::Up), Handled::Ignored);
            assert_eq!(press(shell, t, Key::Down), Handled::Ignored);
        });
    }

    #[test]
    fn the_password_screen_toggles_its_own_mask() {
        with_login(|shell, t| {
            shell.with_app(|a| a.show_password_for_test("dica"));
            frame(shell, t);
            let field = shell.app().login_state().field().expect("the password screen has a field");
            for ch in "hunter2".chars() {
                press(shell, t, Key::Char(ch));
            }
            assert_eq!(field.borrow().display(), "*******");
            press(shell, t, Key::Softkey(Softkey::Left));
            assert_eq!(field.borrow().display(), "hunter2", "the eye did not open");
            press(shell, t, Key::Softkey(Softkey::Left));
            assert_eq!(field.borrow().display(), "*******");
        });
    }

    #[test]
    fn the_field_pastes_because_the_shell_hands_the_bridge_a_clipboard() {
        // A regression waiting to happen. The hand-written screen reached for
        // `symbian_app::SystemClipboard` itself; a declarative field pastes through `KeyCtx`, which
        // holds `NoClipboard` unless the shell hands one over — so the symptom would have been a login
        // field that silently cannot paste, indistinguishable from an empty clipboard.
        let atlases = symbian_preview::Atlases::load();
        atlases.with_themes(|dark, _light| {
            let mut shell = mock_login()
                .with_clipboard(symbian_ui::MemClipboard::with_text("+55 21 99999-0000"));
            shell.with_app(|a| a.show_phone_for_test());
            frame(&mut shell, dark);

            assert_eq!(press(&mut shell, dark, Key::Ctrl('v')), Handled::Consumed);
            let field = shell.app().login_state().field().unwrap();
            // The digits filter is inside the buffer, so the punctuation and the leading `+` — which
            // the screen draws rather than stores — are dropped on the way in.
            assert_eq!(field.borrow().text(), "5521999990000");
        });
    }

    #[test]
    fn the_red_key_leaves_from_every_screen() {
        // It used to be the first thing `App::on_key` did, and the screens that have left the
        // adapter no longer go through that function — so it moved here, and this is what says it
        // arrived. The login code screen is the case that made it urgent: its left softkey is
        // "Voltar", so the *back* slot is empty and `End` would have fallen through to a field that
        // ignores it. A phone that will not close is not a small bug.
        let atlases = symbian_preview::Atlases::load();
        atlases.with_themes(|dark, _light| {
            for (what, mut shell) in [
                ("the dialog list", mock()),
                ("a login screen", mock_login()),
            ] {
                frame(&mut shell, dark);
                assert!(!shell.should_exit(), "{what} wanted to exit before being asked");
                assert_eq!(press(&mut shell, dark, Key::End), Handled::Consumed, "{what}");
                assert!(shell.should_exit(), "{what} did not close on the red key");
            }
        });
    }

    // ---- the conversation -------------------------------------------------------------------------

    /// A shell in the newest chat's conversation, with a frame drawn.
    fn with_conversation(f: impl FnOnce(&mut Shell, &Theme<'_>)) {
        let atlases = symbian_preview::Atlases::load();
        let mut ran = false;
        atlases.with_themes(|dark, _light| {
            let mut shell = mock();
            frame(&mut shell, dark);
            press(&mut shell, dark, Key::Select);
            assert_eq!(shell.app().in_conversation(), Some(0), "the chat did not open");
            frame(&mut shell, dark);
            f(&mut shell, dark);
            ran = true;
        });
        assert!(ran, "the atlases did not yield a theme");
    }

    /// The rows of the softkey bar, which is the part of the screen a stale tree gets wrong.
    fn softkey_rows(shell: &mut Shell, theme: &Theme<'_>) -> alloc::vec::Vec<u16> {
        let mut buf = alloc::vec![0u16; (SCREEN.w * SCREEN.h) as usize];
        let mut c = Canvas::from_slice(&mut buf, SCREEN);
        shell.draw(&mut c, theme);
        let from = ((SCREEN.h - theme.metrics.softkey_h) * SCREEN.w) as usize;
        buf[from..].to_vec()
    }

    #[test]
    fn the_first_character_typed_makes_the_bar_offer_to_send_it() {
        // The bug this test exists for is not in the drawing, it is in *when* the description is
        // rebuilt. Typing is answered by a widget, and the bridge deliberately does not rebuild the
        // view for that — so the "Enviar" label, which lives in the tree, arrived one keypress late
        // or not at all. A pixel comparison of one state cannot see it; the preview, which presses
        // keys and then draws, can. See `conv_decl::ViewState`.
        with_conversation(|shell, t| {
            let empty_bar = softkey_rows(shell, t);
            press(shell, t, Key::Char('o'));
            let typed_bar = softkey_rows(shell, t);
            assert_ne!(empty_bar, typed_bar, "the bar did not gain a label for the text just typed");

            // And it goes away again with the last character.
            press(shell, t, Key::Backspace);
            assert_eq!(softkey_rows(shell, t), empty_bar, "the label outstayed the text");
        });
    }

    #[test]
    fn a_note_the_transcript_wrote_reaches_the_title_bar() {
        // The same staleness, on the other thing `view` reads. Up at the very top of a windowed chat
        // writes "inicio do que esta guardado" and asks for nothing, so there is no action to
        // invalidate the tree — the note has to say so itself.
        with_conversation(|shell, t| {
            shell.with_app(|a| {
                if let Some(c) = a.store.chats.get_mut(0) {
                    c.windowed = true;
                    c.complete = false;
                }
            });
            frame(shell, t);
            // Into the transcript, then to its top.
            press(shell, t, Key::Up);
            for _ in 0..40 {
                press(shell, t, Key::Up);
            }
            let note = shell.app().conversation().and_then(|c| c.note.clone());
            assert_eq!(note.as_deref(), Some(crate::strings::start_of_stored()));
        });
    }

    #[test]
    fn the_action_key_follows_the_focus() {
        // With the composer focused it sends; with the transcript focused it opens what the cursor is
        // on. The bar has one middle slot and cannot say that, so the application routes it — and
        // this is what says the routing is by focus and not by label.
        with_conversation(|shell, t| {
            for ch in "oi".chars() {
                press(shell, t, Key::Char(ch));
            }
            // Focus into the transcript: the action must *not* send now, even though the bar says
            // "Enviar", because that is what the hand-written screen does.
            press(shell, t, Key::Up);
            press(shell, t, Key::Select);
            let text = shell
                .app()
                .conversation()
                .map(|c| c.composer.text().to_string())
                .unwrap_or_default();
            assert_eq!(text, "oi", "the transcript's action key sent the composer's text");

            // Back to the composer, and now it sends.
            let before = shell.app().store.chats[0].messages.len();
            press(shell, t, Key::Down);
            press(shell, t, Key::Select);
            assert_eq!(shell.app().store.chats[0].messages.len(), before + 1);
            assert!(shell.app().conversation().is_some_and(|c| c.composer.is_empty()));
        });
    }

    #[test]
    fn leaving_the_conversation_returns_to_the_chat_it_was_opened_from() {
        with_conversation(|shell, t| {
            press(shell, t, Key::Softkey(Softkey::Right));
            assert!(shell.app().on_chat_list());
            assert_eq!(shell.app().chats_selected, 0);
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
