//! The client's screens, rendered to PNG so a change can be judged before it reaches a phone.
//!
//! ```text
//! cargo run -p tg --example preview      # → preview-out/
//! ```
//!
//! The machinery — the pixel buffer, the atlases chained the way the device chains them, the
//! PNG writer — is `symbian-preview`. What lives here is only the scenes: which screen, in
//! which theme, after which keys. They belong next to the code they document, which is why
//! they are not in the SDK.
//!
//! The chat list and the conversation are reached through `tg::mvu::mock()`, which is what the device
//! runs. The login shots still build a `login::Login` and draw it directly, and that is not a
//! leftover: those screens are pixel-for-pixel identical to the declarative ones — asserted in
//! `examples/login_parity.rs`, in seventeen states — and the imperative side is the one with a
//! constructor for every screen, including the two only a server can produce.

// handle_key and draw are trait methods; the trait must be in scope to call them.
use symbian_ui::App as _;

use symbian_gfx::{Rect, E72_SCREEN};
use symbian_preview::{Atlases, Sheet};

/// Where the sheets land, relative to wherever this was run from.
const OUT: &str = "preview-out";

fn main() {
    let atlases = Atlases::load();
    atlases.with_themes(render);
}

fn render(dark: &symbian_ui::Theme<'_>, light: &symbian_ui::Theme<'_>) {
    let rect = Rect::from_size(E72_SCREEN);

    use symbian_ui::{Key, KeyEvent, Softkey};

    let shot = |name: &str, theme: &symbian_ui::Theme<'_>, keys: &[Key]| {
        let mut app = tg::mvu::mock();
        // A frame before the keys, then one after each: the device draws between batches of events,
        // and a screenshot taken any other way is of a state the phone never shows. It also keeps
        // the scroll offset honest — a list derives it from the selection while being laid out.
        {
            let mut warm = Sheet::new(E72_SCREEN);
            app.draw(&mut warm.canvas(), theme);
            for k in keys {
                app.handle_key(KeyEvent::new(*k), theme, rect);
                app.draw(&mut warm.canvas(), theme);
            }
        }
        let mut s = Sheet::new(E72_SCREEN);
        {
            let mut c = s.canvas();
            app.draw(&mut c, theme);
        }
        s.save(OUT, name);
    };

    shot("10-chats", dark, &[]);
    shot("11-chats-scrolled", dark, &[Key::Down, Key::Down, Key::Down, Key::Down, Key::Down]);
    shot("12-conversation", dark, &[Key::Select]);
    shot("13-transcript-focus", dark, &[Key::Select, Key::Up, Key::Up, Key::Up]);

    // Typing into the composer, to check the caret and the Send softkey appearing.
    let typed: Vec<Key> = "beleza, vou testar".chars().map(Key::Char).collect();
    let mut keys = vec![Key::Down, Key::Down, Key::Select];
    keys.extend(typed);
    shot("14-composing", dark, &keys);

    shot("15-cyrillic", dark, &[Key::Down, Key::Down, Key::Select]);
    // The chat list's Options menu, opened with the left softkey. It had no sheet at all until
    // now, which is how its rows drifted out of step with the list underneath them.
    shot("17-menu", dark, &[Key::Softkey(Softkey::Left)]);
    shot("16-chats-light", light, &[]);

    // The link question, over a real transcript. Drawn here rather than reached through keys
    // because what is being judged is the *overlay* — whether the panel reads as being in front of
    // the conversation — and that is a picture, not a behaviour.
    for (name, theme) in [("17-link-modal", dark), ("18-link-modal-light", light)] {
        let mut app = tg::mvu::mock();
        {
            let mut warm = Sheet::new(E72_SCREEN);
            app.draw(&mut warm.canvas(), theme);
        }
        app.handle_key(KeyEvent::new(Key::Select), theme, rect);
        let mut s = Sheet::new(E72_SCREEN);
        {
            let mut c = s.canvas();
            app.draw(&mut c, theme);
            let mut m = symbian_ui::Modal::new("Abrir link", "https://exemplo.com/uma/pagina?x=1")
                .choice("Copiar e abrir com Web", 0u8)
                .choice("Apenas abrir com Web", 1u8)
                .choice("Copiar link", 2u8);
            m.draw(&mut c, theme);
        }
        s.save(OUT, name);
    }
    let _ = Softkey::Left;

    // Login screens.
    {
        use tg::login::{Login, Screen as Ls};
        let shot_login = |name: &str, theme: &symbian_ui::Theme<'_>, screen: Ls| {
            let mut login = Login::for_preview(screen);
            let mut s = Sheet::new(E72_SCREEN);
            {
                let mut c = s.canvas();
                login.draw(&mut c, theme);
            }
            s.save(OUT, name);
        };
        shot_login("17-login-phone", dark, Ls::Phone {
            field: tg::login::shared(symbian_ui::TextField::with_limit(16)),
            error: None,
        });
        shot_login("18-login-code", dark, Ls::Code {
            field: tg::login::shared(symbian_ui::TextField::with_limit(8)),
            length: Some(5),
            error: None,
        });
        shot_login("19-login-password", dark, Ls::Password {
            field: tg::login::shared({
                let mut f = symbian_ui::TextField::with_limit(128);
                f.set_masked(true);
                f
            }),
            hint: String::from("dica do usuário"),
            error: None,
        });
    }
}
