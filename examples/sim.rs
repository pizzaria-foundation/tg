//! `cargo run -p tg --example sim` — the client in the simulator, on the desktop.
//!
//! Five lines, and that is the point: [`symbian_sim::run`] takes anything implementing
//! `symbian_ui::App`, so a project's runner is this file with one name changed. It lives
//! under `examples/` with `symbian-sim` in `[dev-dependencies]`, which keeps the windowing
//! library away from the device build entirely.
//!
//! The name it changed to is `tg::mvu::mock()`: the client is a model-update-view application behind
//! `symbian-decl-ui`'s bridge now, with the screens that are still hand-written reached through an
//! adapter. The host cannot tell — that is what the bridge is for.

fn main() {
    // A clipboard the host actually has: the shim's is not reachable from a desktop process, so
    // without this copy and paste in the simulator are quiet no-ops and the one path that only paste
    // exercises — a phone number arriving without keystrokes — goes untried until a handset.
    let app = tg::mvu::mock().with_clipboard(symbian_ui::MemClipboard::new());
    if let Err(e) = symbian_sim::run(app) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
