//! The conversation view: a bubble transcript with a composer beneath it.

use alloc::vec::Vec;

use symbian_ui::{
    chrome, Align, Canvas, Frame, Handled, Key, KeyEvent, ListState, Point, Rect, Rows,
    Softkey, TextField, Theme,
};

use crate::model::{Chat, Delivery};

/// One wrapped line, as a byte range into the message text.
struct Line {
    start: usize,
    end: usize,
    width: i32,
}

/// A message with its wrapping resolved for one specific width.
struct Laid {
    lines: Vec<Line>,
    bubble_w: i32,
    height: i32,
    /// True when the timestamp shares the last text line instead of getting its
    /// own. Saves a whole line of vertical space on most messages, which matters
    /// when the transcript viewport is 180px tall.
    time_inline: bool,
    /// Trailing space below this bubble; smaller when the next message is from the
    /// same sender.
    gap: i32,
    /// Extra height for a media placeholder, above the text. Includes [`MEDIA_GAP`].
    media_h: i32,
    /// Byte ranges into `Message::text` that are links, in order.
    ///
    /// Computed here, with the wrapping, rather than in the draw: `Transcript` is rebuilt only when
    /// the width or the message count changes, and scanning every message's text on every frame
    /// would be the same work a hundred times a second to answer a question whose answer cannot
    /// have changed. It is the same bargain the line wrapping already makes.
    links: Vec<core::ops::Range<usize>>,
}

impl Laid {
    /// Height of the media band itself, without the gap under it.
    fn media_band_h(&self) -> i32 {
        (self.media_h - MEDIA_GAP).max(0)
    }

    /// Where the timestamp row starts, measured from the bubble's inner top.
    ///
    /// A method rather than two expressions inside the drawing, because it has to agree
    /// with where the text was actually left — and it did not. The inline branch counted
    /// text lines from `inner.y0` and forgot the media band above them, so on a media
    /// message with little or no text the timestamp and the delivery tick were drawn
    /// straight over the label. Every photo, voice and sticker row showed it.
    fn meta_offset(&self, line_height: i32) -> i32 {
        let lines = self.lines.len() as i32;
        if self.time_inline {
            // Shares the last text line.
            self.media_h + (lines - 1).max(0) * line_height
        } else {
            // Its own line, under everything.
            self.media_h + lines * line_height
        }
    }
}

/// Wrapping is expensive enough that we do it once per (width, message-count)
/// rather than per frame — a 6-message chat at 320px is ~40 `measure` calls, and
/// redraw happens on every keystroke.
pub struct Transcript {
    laid: Vec<Laid>,
    width: i32,
    count: usize,
}

const BUBBLE_HPAD: i32 = 6;
const BUBBLE_VPAD: i32 = 3;
/// Gap between messages from different senders.
const BUBBLE_GAP: i32 = 5;
/// Gap between consecutive messages from the same sender. Grouping them tightly
/// is what makes a transcript read as a conversation rather than a list.
const BUBBLE_GAP_SAME: i32 = 2;
/// Bubbles stop at this fraction of the width so the other party's alignment stays
/// legible even for a long message.
const BUBBLE_MAX_PCT: i32 = 74;
/// Padding above and below the media placeholder's own label.
const MEDIA_VPAD: i32 = 3;
/// Space between the placeholder and the text under it.
const MEDIA_GAP: i32 = 2;

impl Transcript {
    pub fn build(chat: &Chat, theme: &Theme<'_>, avail_w: i32) -> Self {
        let body = theme.fonts.body;
        let small = theme.fonts.small;
        let max_bubble = (avail_w * BUBBLE_MAX_PCT / 100).max(40);
        let inner_max = max_bubble - BUBBLE_HPAD * 2;

        let mut laid = Vec::with_capacity(chat.messages.len());
        for (idx, m) in chat.messages.iter().enumerate() {
            let same_sender = idx > 0 && chat.messages[idx - 1].outgoing == m.outgoing;
            let gap = if same_sender { BUBBLE_GAP_SAME } else { BUBBLE_GAP };
            let mut lines: Vec<Line> = Vec::new();
            body.wrap(&m.text, inner_max, &mut |line: &str| {
                let start = line.as_ptr() as usize - m.text.as_ptr() as usize;
                lines.push(Line { start, end: start + line.len(), width: body.measure(line) });
            });
            if lines.is_empty() {
                lines.push(Line { start: 0, end: 0, width: 0 });
            }

            // The placeholder band plus the gap below it. One number, read by both the
            // layout and the drawing, so they cannot drift apart.
            let media_h = if m.media.is_some() {
                body.line_height() + MEDIA_VPAD * 2 + MEDIA_GAP
            } else {
                0
            };

            // Width of the trailing metadata: time, plus ticks when outgoing.
            let meta_w = small.measure(&m.time)
                + if m.outgoing { small.measure(m.state.glyph()) + 3 } else { 0 };

            let widest = lines.iter().map(|l| l.width).max().unwrap_or(0);
            let last_w = lines.last().map(|l| l.width).unwrap_or(0);

            // Try to tuck the timestamp onto the last line.
            let inline_need = last_w + 6 + meta_w;
            let time_inline = inline_need <= inner_max;
            let (content_w, extra_h) = if time_inline {
                (widest.max(inline_need), 0)
            } else {
                (widest.max(meta_w), small.line_height())
            };

            let height = BUBBLE_VPAD * 2
                + lines.len() as i32 * body.line_height()
                + extra_h
                + gap
                + media_h;

            laid.push(Laid {
                links: symbian::url::find_links(&m.text),
                lines,
                bubble_w: (content_w + BUBBLE_HPAD * 2).min(max_bubble),
                height,
                time_inline,
                gap,
                media_h,
            });
        }

        Self { laid, width: avail_w, count: chat.messages.len() }
    }

    /// True when the cached layout no longer matches what we are about to draw.
    pub fn is_stale(&self, chat: &Chat, avail_w: i32) -> bool {
        self.width != avail_w || self.count != chat.messages.len()
    }
}

impl Rows for Transcript {
    fn len(&self) -> usize {
        self.laid.len()
    }
    fn height(&self, index: usize) -> i32 {
        self.laid[index].height
    }
}

/// Which half of the screen has focus. The composer only receives characters when
/// it is focused, so the D-pad stays available for scrolling the transcript.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Focus {
    Transcript,
    Composer,
}

pub struct Conversation {
    pub chat: usize,
    pub state: ListState,
    pub composer: TextField,
    pub focus: Focus,
    transcript: Option<Transcript>,
    /// Set briefly when the user taps a media message, so the screen can show feedback.
    pub note: Option<alloc::string::String>,
    /// Which link inside the selected message has the cursor, if any.
    ///
    /// Here and not in the model, for the same reason a scroll offset is not in the model: it is a
    /// cursor position, meaningless without the laid-out text beside it, and an `update` that set it
    /// would be guessing at something only the layout knows. The message the cursor is *on* is
    /// `state.selected`, which is model state and stays there.
    link: Option<usize>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConvAction {
    Back,
    Send(alloc::string::String),
    /// Scroll reached the top of the transcript; request the page above.
    LoadMore,
    /// The user pressed Select on a message with media.
    OpenMedia(usize),
    /// The user pressed Select on a focused link. Carries the URL as written in the message —
    /// the caller decides what opens it, which on this device means asking the launcher.
    OpenLink(alloc::string::String),
    /// Re-fetch the newest page of this conversation.
    Refresh,
    /// The user asked to copy what is highlighted. Carries the text already resolved to whatever
    /// the cursor was on, which is the same division of labour [`ConvAction::OpenLink`] draws:
    /// this screen knows *what* was meant, the application owns the clipboard.
    Copy(alloc::string::String),
    None,
}

impl Conversation {
    pub fn new(chat: usize) -> Self {
        Self {
            chat,
            state: ListState::new(),
            // Telegram's own limit; also stops one message from wrapping forever.
            composer: TextField::with_limit(4096),
            focus: Focus::Composer,
            transcript: None,
            note: None,
            link: None,
        }
    }

    fn composer_h(&self, theme: &Theme<'_>) -> i32 {
        theme.fonts.body.line_height() + 8
    }

    /// Re-wrap if the width or the message count changed, then hand back the
    /// layout along with the transcript viewport height.
    fn ensure_layout(&mut self, chat: &Chat, theme: &Theme<'_>, screen: Rect) -> (Rect, Rect) {
        let frame = Frame::split(screen, theme, true, true);
        let (composer, transcript_area) = frame.content.split_bottom(self.composer_h(theme));
        let avail = transcript_area.width() - theme.metrics.pad * 2;

        let stale = self.transcript.as_ref().is_none_or(|t| t.is_stale(chat, avail));
        if stale {
            // Save which message is at the top of the viewport so we can restore it
            // after the rebuild. Without this, new messages prepended by lazy loading
            // shift everything down and the user jumps to a different point.
            let saved_top = self.transcript.as_ref()
                .and_then(|t| ListState::row_at(t, self.state.scroll));
            let saved_selected = self.state.selected;

            let t = Transcript::build(chat, theme, avail);
            // A fresh conversation opens at the newest message, which is what a
            // chat client must always do.
            let first_build = self.transcript.is_none();
            self.transcript = Some(t);
            if first_build {
                let t = self.transcript.as_ref().unwrap();
                self.state.scroll_to_end(t, transcript_area.height());
            } else {
                let t = self.transcript.as_ref().unwrap();
                // Restore the position: find the same message index in the new layout
                // and set scroll to its top. The selected index is restored as well.
                if let Some(idx) = saved_top {
                    if idx < t.len() {
                        self.state.selected = saved_selected.min(t.len().saturating_sub(1));
                        self.state.scroll = ListState::row_top(t, self.state.selected);
                    } else {
                        self.state.clamp(t, transcript_area.height());
                    }
                } else {
                    self.state.clamp(t, transcript_area.height());
                }
            }
        }
        (transcript_area, composer)
    }

    /// The links in the selected message, if the layout has been built.
    fn links_here(&self) -> &[core::ops::Range<usize>] {
        self.transcript
            .as_ref()
            .and_then(|t| t.laid.get(self.state.selected))
            .map(|l| l.links.as_slice())
            .unwrap_or(&[])
    }

    /// The focused link's text, if there is one.
    pub fn focused_link<'a>(&self, chat: &'a Chat) -> Option<&'a str> {
        let i = self.link?;
        let r = self.links_here().get(i)?.clone();
        chat.messages.get(self.state.selected)?.text.get(r)
    }

    /// Move the link cursor by `delta`, returning whether that consumed the key.
    ///
    /// The model is one linear sequence of stops, walked forwards by Down and backwards by Up:
    ///
    /// ```text
    ///   [message A] [A link 0] [A link 1] … [message B] [B link 0] …
    /// ```
    ///
    /// So a message is a stop in its own right — which is what keeps Select opening the *media* on
    /// a message that has both — and its links are the stops after it. `false` means the cursor ran
    /// off the end of this message's links and the caller should move the selection instead.
    ///
    /// The one asymmetry is deliberate and is what makes the sequence reversible: arriving at a
    /// message from *below* lands on its last link, not on the message. Anything else would make
    /// Up unable to reach a link it had just walked down through.
    fn step_link(&mut self, delta: isize) -> bool {
        let n = self.links_here().len();
        if n == 0 {
            return false;
        }
        match (self.link, delta) {
            (None, 1) => {
                self.link = Some(0);
                true
            }
            (Some(i), 1) if i + 1 < n => {
                self.link = Some(i + 1);
                true
            }
            (Some(i), -1) if i > 0 => {
                self.link = Some(i - 1);
                true
            }
            // Up off the first link lands on the message, which is the stop before its links — so
            // the key is consumed and the selection does not move.
            (Some(0), -1) => {
                self.link = None;
                true
            }
            // Down off the last link leaves the message entirely: the next stop is the next
            // message, so the cursor is dropped and the selection moves.
            (Some(_), _) => {
                self.link = None;
                false
            }
            (None, _) => false,
        }
    }

    /// Land the link cursor after the selection moved, so the sequence reads the same backwards.
    fn enter_from(&mut self, delta: isize) {
        self.link = if delta < 0 {
            // Came up into this message: its last link is the stop just before the message itself.
            match self.links_here().len() {
                0 => None,
                n => Some(n - 1),
            }
        } else {
            None
        };
    }

    /// Route a key, giving the link cursor first refusal on Up and Down.
    ///
    /// A wrapper rather than a branch inside the routing below, and that is the whole point: the
    /// existing Up/Down arms carry real behaviour — asking for older messages at the top, handing
    /// focus to the composer at the bottom — and re-implementing any of it here to make room for a
    /// link cursor would be a second copy that drifts. So the cursor walks first, and if it has run
    /// out of links the untouched routing runs exactly as before.
    pub fn handle_key(
        &mut self,
        ev: KeyEvent,
        chat: &Chat,
        theme: &Theme<'_>,
        screen: Rect,
    ) -> (Handled, ConvAction) {
        let delta = match ev.key {
            Key::Up if self.focus == Focus::Transcript => Some(-1isize),
            Key::Down if self.focus == Focus::Transcript => Some(1isize),
            _ => None,
        };
        let Some(d) = delta else {
            return self.route_key(ev, chat, theme, screen);
        };
        // The layout has to exist before `links_here` can answer, and it is what the routing builds
        // first anyway.
        self.ensure_layout(chat, theme, screen);
        if self.step_link(d) {
            return (Handled::Consumed, ConvAction::None);
        }
        let before = self.state.selected;
        let out = self.route_key(ev, chat, theme, screen);
        if self.state.selected != before {
            self.enter_from(d);
        }
        out
    }

    fn route_key(
        &mut self,
        ev: KeyEvent,
        chat: &Chat,
        theme: &Theme<'_>,
        screen: Rect,
    ) -> (Handled, ConvAction) {
        let (area, _) = self.ensure_layout(chat, theme, screen);
        let vp = area.height();

        match ev.key {
            Key::Softkey(Softkey::Right) => return (Handled::Consumed, ConvAction::Back),
            // Re-fetch this conversation. Nothing here is pushed, so a reply that arrived
            // while the screen was open is invisible until something asks for it.
            Key::Softkey(Softkey::Left) => {
                self.note = Some(alloc::string::String::from("atualizando…"));
                return (Handled::Consumed, ConvAction::Refresh);
            }
            Key::Softkey(Softkey::Middle) | Key::Enter | Key::Call | Key::Select => {
                // In Transcript focus, open media or ignore — never send text.
                if self.focus == Focus::Transcript {
                    // A focused link wins over the message's media. It has to: the cursor is
                    // visibly on the link, and opening something else would be the screen doing one
                    // thing while showing another.
                    if let Some(url) = self.focused_link(chat) {
                        let url = alloc::string::String::from(url);
                        self.note = Some(alloc::format!("abrindo {url}"));
                        return (Handled::Consumed, ConvAction::OpenLink(url));
                    }
                    if let Some(msg) = chat.messages.get(self.state.selected) {
                        if let Some(media) = &msg.media {
                            let label = media_label(media, theme.fonts.body);
                            self.note = Some(alloc::format!("abrindo {label}…"));
                            return (Handled::Consumed, ConvAction::OpenMedia(self.state.selected));
                        }
                    }
                    return (Handled::Consumed, ConvAction::None);
                }
                // Composer focus: send if not empty.
                if !self.composer.is_empty() {
                    return (Handled::Consumed, ConvAction::Send(self.composer.take()));
                }
                return (Handled::Consumed, ConvAction::None);
            }
            // Up out of the composer moves focus into the transcript; Down at the
            // end of the transcript hands focus back.
            Key::Up if self.focus == Focus::Composer => {
                self.focus = Focus::Transcript;
                return (Handled::Consumed, ConvAction::None);
            }
            // Up at the top of the transcript asks for older messages. Checked
            // before the focus dispatch so ListState's handle_key — which would
            // consume it without moving — does not shadow the trigger.
            Key::Up
                if self.focus == Focus::Transcript
                    && self.state.selected == 0
                    && self.state.scroll == 0 =>
            {
                // The window holds the newest hundred and drops the rest, so above them
                // there is nothing to fetch — asking would spend a request on a page that
                // is discarded on arrival. Say so instead of appearing to hang.
                if chat.windowed && !chat.complete {
                    self.note = Some(alloc::string::String::from("inicio do que esta guardado"));
                    return (Handled::Consumed, ConvAction::None);
                }
                return (Handled::Consumed, ConvAction::LoadMore);
            }
            Key::Down if self.focus == Focus::Transcript => {
                let t = self.transcript.as_ref().unwrap();
                if self.state.selected + 1 >= t.len() {
                    self.focus = Focus::Composer;
                    return (Handled::Consumed, ConvAction::None);
                }
            }
            // Ctrl+C on the transcript copies what the cursor is on. The focused link wins over the
            // message for the same reason it wins on Select: the cursor is visibly on the link, and
            // copying the whole message instead would be the screen doing one thing while showing
            // another. In the composer this key never arrives here — the field answers it first,
            // where it copies the selection.
            Key::Ctrl('c') if self.focus == Focus::Transcript => {
                if let Some(url) = self.focused_link(chat) {
                    let url = alloc::string::String::from(url);
                    self.note = Some(alloc::format!("copiado: {url}"));
                    return (Handled::Consumed, ConvAction::Copy(url));
                }
                let text = chat
                    .messages
                    .get(self.state.selected)
                    .map(|m| m.text.clone())
                    .unwrap_or_default();
                if text.is_empty() {
                    // A photo with no caption. Saying nothing happened beats a "copiado" over an
                    // empty clipboard.
                    self.note = Some(alloc::string::String::from("nada para copiar"));
                    return (Handled::Consumed, ConvAction::None);
                }
                self.note = Some(alloc::string::String::from("mensagem copiada"));
                return (Handled::Consumed, ConvAction::Copy(text));
            }
            // Left/Right in Transcript: move one message, not a page jump.
            Key::Left if self.focus == Focus::Transcript => {
                let t = self.transcript.as_ref().unwrap();
                self.state.move_selection(-1, t, vp);
                return (Handled::Consumed, ConvAction::None);
            }
            Key::Right if self.focus == Focus::Transcript => {
                let t = self.transcript.as_ref().unwrap();
                self.state.move_selection(1, t, vp);
                return (Handled::Consumed, ConvAction::None);
            }
            _ => {}
        }

        let handled = match self.focus {
            // The clipboard the field pastes from and copies to. Handing it over here is all an
            // app has to do for Ctrl+C/X/V and Shift+arrow selection to work in every one of its
            // fields — the editing itself is the toolkit's.
            Focus::Composer => self.composer.handle_key(ev, &mut symbian_app::SystemClipboard),
            Focus::Transcript => {
                let t = self.transcript.as_ref().unwrap();
                self.state.handle_key(ev, t, vp)
            }
        };
        (handled, ConvAction::None)
    }

    pub fn draw(&mut self, c: &mut Canvas<'_>, chat: &Chat, theme: &Theme<'_>) {
        let screen = Rect::from_size(c.size());
        let (area, composer_r) = self.ensure_layout(chat, theme, screen);
        let frame = Frame::split(screen, theme, true, true);
        let t = self.transcript.as_ref().unwrap();
        let p = &theme.palette;

        chrome::clear(c, theme);
        let sub = if chat.loading { Some("carregando…") } else { self.note.as_deref() };
        chrome::title_bar(c, frame.title, theme, &chat.name, sub);

        let bar = self.state.scrollbar(t, area.height());
        let gutter = chrome::scrollbar_gutter(theme, bar.is_some());
        // A short conversation hangs from the composer rather than the title bar.
        // Every chat client does this, and the alternative — a wall of empty space
        // below two messages — reads as a rendering fault.
        let anchor = (area.height() - ListState::content_height(t)).max(0);
        let body = Rect {
            x0: area.x0 + theme.metrics.pad,
            y0: area.y0 + anchor,
            x1: area.x1 - theme.metrics.pad - gutter,
            ..area
        };

        // The clip is `draw_visible`'s, not ours — see the note in `chats.rs`.
        self.state.draw_visible(c, t, body, |c, i, r| {
            let focused = self.focus == Focus::Transcript && i == self.state.selected;
            // The link cursor belongs to the selected message and to no other, so a link
            // highlighted on a row the cursor has left is not possible by construction.
            let link_focus = if focused { self.link } else { None };
            draw_bubble(c, r, theme, &chat.messages[i], &t.laid[i], focused, link_focus);
        });
        chrome::scrollbar(c, area, theme, bar);

        // Composer
        c.hline(composer_r.y0, composer_r.x0, composer_r.x1, p.divider);
        symbian_ui::paint::band(c, Rect { y0: composer_r.y0 + 1, ..composer_r }, &p.chrome);
        let field = composer_r.inset_xy(theme.metrics.pad, 3);
        let focused = self.focus == Focus::Composer;

        if self.composer.is_empty() {
            c.draw_text_in(field, "Mensagem…", theme.fonts.body, p.dim, Align::Start);
        } else {
            // Under the text, so the characters stay on top of their own highlight.
            if let Some((from, to)) = self.composer.selection() {
                symbian_ui::paint::text_selection(
                    c,
                    field.x0,
                    field.y0,
                    field.y1,
                    self.composer.text(),
                    from,
                    to,
                    theme.fonts.body,
                    p.selection.mid(),
                );
            }
            c.draw_text_in(field, self.composer.text(), theme.fonts.body, p.text, Align::Start);
        }
        if focused {
            // Caret at the measured width of the text before the cursor, so it
            // tracks the insertion point rather than always sitting at the end.
            let before = &self.composer.text()[..self.composer.cursor()];
            let cx = field.x0 + theme.fonts.body.measure(before);
            c.fill_rect(Rect::new(cx, field.y0, cx + 1, field.y1), p.accent);
        }

        let send = if self.composer.is_empty() { None } else { Some("Enviar") };
        chrome::softkey_bar(c, frame.softkeys, theme, [Some("Atualizar"), send, Some("Voltar")]);
    }
}

/// Draw one wrapped line, breaking it where links start and end.
///
/// The alternative — draw the line, then paint link decoration over it — cannot work: the toolkit
/// draws a run of text from a point and the only way to know where a substring lands is to have
/// measured everything before it. So the line is walked in runs, each drawn at the pen where the
/// previous one left it, which is the same walk a browser does and the reason a link that wraps
/// underlines on both lines without anything having to know it wrapped.
///
/// The focused link is inverted rather than merely coloured. On a 320x240 screen at this font size
/// a colour change is not a cursor — it reads as emphasis, and the user cannot tell what Select
/// would open. A filled block can only mean "here".
#[allow(clippy::too_many_arguments)]
fn draw_text_line(
    c: &mut Canvas<'_>,
    theme: &Theme<'_>,
    x0: i32,
    y: i32,
    text: &str,
    line: &Line,
    links: &[core::ops::Range<usize>],
    link_focus: Option<usize>,
    fg: symbian_ui::Color,
) {
    let body = theme.fonts.body;
    let p = &theme.palette;
    let baseline = y + body.ascent();
    let mut pen = x0;
    let mut at = line.start;

    // Walk the line's byte range, emitting the gap before each overlapping link and then the link
    // itself. `links` is ordered and non-overlapping — `symbian::url::find_links` guarantees both —
    // so one pass is enough and no run can be visited twice.
    for (i, link) in links.iter().enumerate() {
        if link.end <= at || link.start >= line.end {
            continue;
        }
        let lo = link.start.max(at);
        let hi = link.end.min(line.end);
        if lo > at {
            let run = &text[at..lo];
            c.draw_text(Point::new(pen, baseline), run, body, fg);
            pen += body.measure(run);
        }
        let run = &text[lo..hi];
        let w = body.measure(run);
        if link_focus == Some(i) {
            // Inverted: the accent as ground, the bubble's own text colour as figure. Drawn a pixel
            // proud of the glyphs on every side so the block reads as a selection and not as a
            // highlighter that clipped the descenders.
            c.fill_rect(Rect::new(pen - 1, y, pen + w + 1, y + body.line_height()), p.accent);
            c.draw_text(Point::new(pen, baseline), run, body, p.accent_text);
        } else {
            c.draw_text(Point::new(pen, baseline), run, body, p.accent);
            // The underline is what says "link" when the accent is close to the text colour, which
            // it is on some palettes. One pixel below the baseline, not below the line box: under
            // the line box it would sit against the next row of text.
            c.hline(baseline + 1, pen, pen + w, p.accent);
        }
        pen += w;
        at = hi;
    }
    if at < line.end {
        c.draw_text(Point::new(pen, baseline), &text[at..line.end], body, fg);
    }
}

/// Whether the atlas can actually draw every character of `s`.
///
/// A missing codepoint is not a visible box here: `mkfont.py` deliberately drops glyphs the
/// source font does not have rather than shipping `.notdef`, so the row is charged
/// `fallback_advance` and *nothing is painted*. A label made of characters outside the
/// atlas therefore renders as blank space, which reads as a bug in the layout rather than
/// as a missing font.
///
/// This matters for exactly one thing: a sticker's `alt` is an emoji, and the device
/// atlases carry ASCII, Latin-1, Cyrillic and a handful of punctuation — no emoji at all.
/// See `tools/mkfont.py`'s `default_charset`.
fn renderable(font: &dyn symbian_ui::Font, s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| font.glyph(c).is_some())
}

/// What a media row says before anything is loaded.
///
/// A label, never a picture: nothing is fetched *or decoded* until the user presses Select,
/// including previews that arrived inside the message. So the label has to carry enough to
/// decide with — how long the voice note runs, how large the file is — and to say that
/// there is something to open.
fn media_label(media: &crate::model::Media, font: &dyn symbian_ui::Font) -> alloc::string::String {
    use crate::model::Media;
    // A glyph for the kind, and the word when the glyph is not in the atlas. The mark this
    // used to draw was U+266A EIGHTH NOTE, which is in *none* of the three text fonts nor
    // in Noto Emoji — so every voice row rendered as "[ 0:07]" with a hole in it, and only
    // a rendered screenshot showed it. `mark` therefore never assumes.
    let mark = |glyph: char, word: &str| -> alloc::string::String {
        if font.glyph(glyph).is_some() {
            alloc::string::String::from(glyph)
        } else {
            alloc::string::String::from(word)
        }
    };
    match media {
        Media::Photo { size, .. } => {
            let m = mark('\u{1F5BC}', "Foto");
            if *size > 0 {
                alloc::format!("[{m} {}]", size_fmt(*size))
            } else {
                alloc::format!("[{m}]")
            }
        }
        // The emoji from documentAttributeSticker, but only when the atlas has it. It
        // almost never does, and the alternative to checking is a bubble that draws an
        // empty box.
        Media::Sticker { alt, .. } => {
            if renderable(font, alt) {
                alloc::format!("[{alt}]")
            } else {
                alloc::string::String::from("[Sticker]")
            }
        }
        // A microphone for a voice note, a note for a music file: the same distinction the
        // `voice` flag makes in the protocol, which is why reading that flag mattered.
        Media::Voice { duration, .. } => {
            alloc::format!("[{} {}]", mark('\u{1F3A4}', "Voz"), mmss(*duration))
        }
        Media::Audio { filename, duration, .. } => {
            let m = mark('\u{1F3B5}', "Audio");
            if filename.is_empty() {
                alloc::format!("[{m} {}]", mmss(*duration))
            } else {
                alloc::format!("[{m} {filename} · {}]", mmss(*duration))
            }
        }
        Media::File { filename, size, .. } => {
            let m = mark('\u{1F4CE}', "Arquivo");
            if filename.is_empty() {
                alloc::format!("[{m} {}]", size_fmt(*size))
            } else {
                alloc::format!("[{m} {filename} · {}]", size_fmt(*size))
            }
        }
        Media::Unknown => alloc::string::String::from("[Midia]"),
    }
}

/// `M:SS`, or `H:MM:SS` for anything over an hour.
fn mmss(seconds: i32) -> alloc::string::String {
    let s = seconds.max(0);
    if s >= 3600 {
        alloc::format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        alloc::format!("{}:{:02}", s / 60, s % 60)
    }
}

fn size_fmt(bytes: i64) -> alloc::string::String {
    if bytes < 1024 { return alloc::format!("{bytes}") }
    if bytes < 1024 * 1024 { return alloc::format!("{} KB", bytes / 1024) }
    alloc::format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

fn draw_bubble(
    c: &mut Canvas<'_>,
    row: Rect,
    theme: &Theme<'_>,
    m: &crate::model::Message,
    laid: &Laid,
    focused: bool,
    // Which of this message's links has the cursor, if this is the selected message.
    link_focus: Option<usize>,
) {
    let p = &theme.palette;
    let body = theme.fonts.body;
    let small = theme.fonts.small;

    let h = row.height() - laid.gap;
    let bubble = if m.outgoing {
        Rect::from_xywh(row.x1 - laid.bubble_w, row.y0, laid.bubble_w, h)
    } else {
        Rect::from_xywh(row.x0, row.y0, laid.bubble_w, h)
    };

    let (fill, fg): (symbian_ui::Surface, _) = if m.outgoing {
        (p.bubble_out, p.bubble_out_text)
    } else {
        (p.bubble_in, p.bubble_in_text)
    };
    symbian_ui::paint::band_round(c, bubble, &fill, theme.metrics.radius);
    if focused {
        // Outline rather than a fill swap, so the incoming/outgoing colour still
        // reads while the row has focus.
        c.stroke_rect(bubble, p.accent);
    }

    let inner = bubble.inset_xy(BUBBLE_HPAD, BUBBLE_VPAD);
    let mut y = inner.y0;

    if let Some(media) = &m.media {
        // A label, and nothing else. No thumbnail is drawn here even when the message
        // brought one inline: loading is the user's decision, and a transcript that
        // decoded a preview per row would spend the codec's four slots and the heap on
        // pictures nobody asked to see.
        let label = media_label(media, body);
        let lw = body.measure(&label);
        // The height the layout reserved, less the gap it also budgeted for. Taken from
        // `laid` rather than recomputed: the two used to disagree by two pixels, with the
        // layout reserving `line_height + 8` and the drawing using `+ 6`, and nothing would
        // have caught it growing into a real overlap.
        let ph = Rect::from_xywh(inner.x0, y, inner.width(), laid.media_band_h());
        // Subtle fill behind the placeholder so it reads as a distinct target.
        symbian_ui::paint::band(c, ph, &theme.palette.chrome);
        let lx = ph.x0 + (ph.width() - lw) / 2;
        let ly = ph.y0 + (ph.height() - body.line_height()) / 2 + body.ascent();
        c.draw_text(symbian_ui::Point::new(lx.max(ph.x0 + 4), ly), &label, body, fg);
        y = ph.y1 + MEDIA_GAP;
    }
    for l in &laid.lines {
        draw_text_line(c, theme, inner.x0, y, &m.text, l, &laid.links, link_focus, fg);
        y += body.line_height();
    }

    // Metadata: dimmed against the bubble fill rather than the page background.
    let meta_col = if m.outgoing { p.accent_text.with_alpha(0xB0) } else { p.dim };
    let meta_y = inner.y0 + laid.meta_offset(body.line_height());
    let meta_row = Rect::new(inner.x0, meta_y, inner.x1, meta_y + body.line_height());

    let mut right = inner.x1;
    if m.outgoing {
        let g = m.state.glyph();
        let gw = small.measure(g);
        let col = match m.state {
            Delivery::Failed => p.unread,
            Delivery::Read => p.accent_text,
            _ => meta_col,
        };
        c.draw_text_in(Rect { x0: right - gw, ..meta_row }, g, small, col, Align::End);
        right -= gw + 3;
    }
    let tw = small.measure(&m.time);
    c.draw_text_in(
        Rect { x0: right - tw, x1: right, ..meta_row },
        &m.time,
        small,
        meta_col,
        Align::End,
    );
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::Store;
    // `Font` for its `glyph`: the sticker-label test asserts the atlas genuinely lacks the
    // emoji it is falling back from, so the fixture cannot drift away from the device.
    use symbian_ui::{BitmapFont, Font as _, Fonts, Size};

    pub(crate) fn theme_with<'a>(f: &'a BitmapFont<'a>) -> Theme<'a> {
        Theme::dark(Fonts { body: f, strong: f, small: f, title: f })
    }

    pub(crate) fn atlas() -> alloc::vec::Vec<u8> {
        // Every glyph 6x8, advance 6, so widths are trivially predictable.
        let chars: alloc::vec::Vec<char> = (0x20u32..0x500)
            .filter_map(char::from_u32)
            .collect();
        let mut idx = alloc::vec::Vec::new();
        let mut blob = alloc::vec::Vec::new();
        for ch in &chars {
            idx.extend_from_slice(&(*ch as u32).to_le_bytes());
            idx.extend_from_slice(&(blob.len() as u32).to_le_bytes());
            idx.extend_from_slice(&[6, 8, 6, 0]);
            idx.extend_from_slice(&0i16.to_le_bytes());
            idx.extend_from_slice(&8i16.to_le_bytes());
            blob.extend(core::iter::repeat(0x80u8).take(48));
        }
        let mut v = alloc::vec::Vec::new();
        v.extend_from_slice(b"SBF1");
        v.extend_from_slice(&12u16.to_le_bytes());
        v.extend_from_slice(&9i16.to_le_bytes());
        v.extend_from_slice(&3i16.to_le_bytes());
        v.extend_from_slice(&(chars.len() as u16).to_le_bytes());
        v.push(1);
        v.push(6);
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&idx);
        v.extend_from_slice(&blob);
        v
    }

    #[test]
    fn every_line_fits_inside_its_bubble() {
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let store = Store::mock();
        let avail = 320 - t.metrics.pad * 2;

        for chat in &store.chats {
            let tr = Transcript::build(chat, &t, avail);
            for (m, l) in chat.messages.iter().zip(&tr.laid) {
                let inner = l.bubble_w - BUBBLE_HPAD * 2;
                for line in &l.lines {
                    assert!(
                        line.width <= inner,
                        "line {:?} is {}px in a {}px bubble",
                        &m.text[line.start..line.end],
                        line.width,
                        inner
                    );
                }
                assert!(l.bubble_w <= avail * BUBBLE_MAX_PCT / 100 + 1);
            }
        }
    }

    #[test]
    fn wrapping_preserves_every_character() {
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let store = Store::mock();
        let tr = Transcript::build(&store.chats[0], &t, 200);

        for (m, l) in store.chats[0].messages.iter().zip(&tr.laid) {
            let joined: alloc::string::String =
                l.lines.iter().map(|ln| &m.text[ln.start..ln.end]).collect();
            let expect: alloc::string::String =
                m.text.chars().filter(|c| *c != ' ' && *c != '\n').collect();
            let got: alloc::string::String = joined.chars().filter(|c| *c != ' ').collect();
            assert_eq!(got, expect, "text lost while wrapping {:?}", m.text);
        }
    }

    #[test]
    fn transcript_opens_at_the_newest_message() {
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let store = Store::mock();
        let mut conv = Conversation::new(0);
        let screen = Rect::from_size(Size::new(320, 240));
        conv.ensure_layout(&store.chats[0], &t, screen);

        let tr = conv.transcript.as_ref().unwrap();
        assert_eq!(conv.state.selected, tr.len() - 1);
        // And is scrolled as far as the content allows.
        let vp = Frame::split(screen, &t, true, true)
            .content
            .split_bottom(conv.composer_h(&t))
            .1
            .height();
        let expect = (ListState::content_height(tr) - vp).max(0);
        assert_eq!(conv.state.scroll, expect);
    }

    #[test]
    fn right_softkey_goes_back_and_middle_sends() {
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let store = Store::mock();
        let screen = Rect::from_size(Size::new(320, 240));
        let mut conv = Conversation::new(0);

        let (_, a) = conv.handle_key(KeyEvent::new(Key::Softkey(Softkey::Right)), &store.chats[0], &t, screen);
        assert!(matches!(a, ConvAction::Back));

        // Empty composer: middle softkey must not emit an empty message.
        let (_, a) = conv.handle_key(KeyEvent::new(Key::Softkey(Softkey::Middle)), &store.chats[0], &t, screen);
        assert!(matches!(a, ConvAction::None));

        for ch in "oi".chars() {
            conv.handle_key(KeyEvent::new(Key::Char(ch)), &store.chats[0], &t, screen);
        }
        let (_, a) = conv.handle_key(KeyEvent::new(Key::Softkey(Softkey::Middle)), &store.chats[0], &t, screen);
        match a {
            ConvAction::Send(s) => assert_eq!(s, "oi"),
            _ => panic!("expected a send"),
        }
        assert!(conv.composer.is_empty(), "composer must clear after sending");
    }

    #[test]
    fn the_timestamp_never_lands_on_top_of_the_media_label() {
        // What the preview caught: the inline-timestamp row was measured from the top of the
        // bubble's text, ignoring the media band above it — so on a photo, voice or sticker
        // row the time and the delivery tick were drawn straight over the label. Visible on
        // every single media message.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let store = Store::mock();
        let screen = Rect::from_size(Size::new(320, 240));
        let chat = &store.chats[0];
        let tr = Transcript::build(chat, &t, screen.width());

        let lh = t.fonts.body.line_height();
        let mut checked = 0;
        for (i, m) in chat.messages.iter().enumerate() {
            let laid = &tr.laid[i];
            if m.media.is_none() {
                assert_eq!(laid.media_h, 0, "no media, no reserved band");
                continue;
            }
            checked += 1;
            assert!(laid.media_band_h() > 0, "a media row reserves a band");
            assert!(
                laid.meta_offset(lh) >= laid.media_band_h(),
                "message {i}: timestamp at {} overlaps a band ending at {}",
                laid.meta_offset(lh),
                laid.media_band_h(),
            );
            // And everything still fits inside the height the layout claimed, which is what
            // the scroll arithmetic upstream trusts.
            assert!(
                laid.meta_offset(lh) + lh + BUBBLE_VPAD * 2 + laid.gap <= laid.height,
                "message {i} overflows its own bubble",
            );
        }
        assert!(checked >= 3, "the mock must carry media rows to test; found {checked}");
    }

    #[test]
    fn a_sticker_label_does_not_rely_on_an_emoji_the_atlas_lacks() {
        // The device atlases carry ASCII, Latin-1 and Cyrillic — no emoji. A missing glyph
        // paints nothing and only advances, so putting the sticker's `alt` in the label
        // unconditionally would draw an empty box that reads as a layout bug.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let sticker = crate::model::Media::Sticker {
            id: 1,
            access_hash: 2,
            file_reference: alloc::vec::Vec::new(),
            dc_id: 2,
            alt: alloc::string::String::from("\u{1F600}"),
            preview: None,
        };
        // The test atlas covers 0x20..0x500, which excludes U+1F600 just as the real ones do.
        assert!(f.glyph('\u{1F600}').is_none(), "the fixture matches the device here");
        assert_eq!(media_label(&sticker, &f), "[Sticker]");

        // And when the emoji *is* drawable, it is used — so adding emoji to the atlas is all
        // it would take, with no change here.
        let ascii = crate::model::Media::Sticker {
            id: 1,
            access_hash: 2,
            file_reference: alloc::vec::Vec::new(),
            dc_id: 2,
            alt: alloc::string::String::from(":)"),
            preview: None,
        };
        assert_eq!(media_label(&ascii, &f), "[:)]");
    }

    #[test]
    fn a_media_bubble_shows_a_label_and_never_a_picture() {
        // Loading is the user's decision. Even a message carrying a complete inline JPEG
        // draws a label, because decoding one per visible row would spend the codec's four
        // slots and the heap on pictures nobody asked to see.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let with_preview = crate::model::Media::Photo {
            id: 1,
            access_hash: 2,
            file_reference: alloc::vec::Vec::new(),
            dc_id: 2,
            thumb_size: alloc::string::String::from("m"),
            size: 12_000,
            preview: Some(alloc::vec![0xFF, 0xD8, 0xFF, 0xE0]),
        };
        let label = media_label(&with_preview, &f);
        assert!(label.starts_with("[Foto"), "still a label: {label}");
        assert!(label.contains("KB"), "and it says what opening would cost: {label}");
    }

    #[test]
    fn the_left_softkey_refreshes_without_disturbing_back_or_send() {
        // There is no push in this client, so a screen is only as fresh as its last request.
        // The left softkey was empty on every screen; the right one is Back and must stay
        // Back, because that is where S60 puts it.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let store = Store::mock();
        let screen = Rect::from_size(Size::new(320, 240));
        let mut conv = Conversation::new(0);

        let (h, a) = conv.handle_key(
            KeyEvent::new(Key::Softkey(Softkey::Left)), &store.chats[0], &t, screen,
        );
        assert!(matches!(h, Handled::Consumed));
        assert!(matches!(a, ConvAction::Refresh));
        assert!(conv.note.is_some(), "the user gets told something is happening");

        // And the other two are unchanged.
        let (_, a) = conv.handle_key(
            KeyEvent::new(Key::Softkey(Softkey::Right)), &store.chats[0], &t, screen,
        );
        assert!(matches!(a, ConvAction::Back));
    }

    #[test]
    fn refreshing_does_not_send_the_composer_text() {
        // The composer and the refresh share a screen; asking for new messages must not
        // post what is half-typed.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let store = Store::mock();
        let screen = Rect::from_size(Size::new(320, 240));
        let mut conv = Conversation::new(0);
        for ch in "rascunho".chars() {
            conv.handle_key(KeyEvent::new(Key::Char(ch)), &store.chats[0], &t, screen);
        }
        let (_, a) = conv.handle_key(
            KeyEvent::new(Key::Softkey(Softkey::Left)), &store.chats[0], &t, screen,
        );
        assert!(matches!(a, ConvAction::Refresh));
        assert_eq!(conv.composer.text(), "rascunho", "the draft survives");
    }

    #[test]
    fn focus_moves_between_composer_and_transcript() {
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let store = Store::mock();
        let screen = Rect::from_size(Size::new(320, 240));
        let mut conv = Conversation::new(0);

        assert_eq!(conv.focus, Focus::Composer);
        conv.handle_key(KeyEvent::new(Key::Up), &store.chats[0], &t, screen);
        assert_eq!(conv.focus, Focus::Transcript);
        // Already on the last message, so Down hands focus straight back.
        conv.handle_key(KeyEvent::new(Key::Down), &store.chats[0], &t, screen);
        assert_eq!(conv.focus, Focus::Composer);
    }

    #[test]
    fn typing_while_the_transcript_has_focus_does_not_edit_the_composer() {
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let store = Store::mock();
        let screen = Rect::from_size(Size::new(320, 240));
        let mut conv = Conversation::new(0);
        conv.focus = Focus::Transcript;
        conv.handle_key(KeyEvent::new(Key::Char('x')), &store.chats[0], &t, screen);
        assert!(conv.composer.is_empty());
    }

    #[test]
    fn relayout_after_a_new_message_keeps_scroll_valid() {
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let mut store = Store::mock();
        let screen = Rect::from_size(Size::new(320, 240));
        let mut conv = Conversation::new(0);
        conv.ensure_layout(&store.chats[0], &t, screen);

        store.chats[0].messages.truncate(1);
        conv.ensure_layout(&store.chats[0], &t, screen);
        let tr = conv.transcript.as_ref().unwrap();
        assert_eq!(tr.len(), 1);
        assert!(conv.state.scroll >= 0);
        assert!(conv.state.selected < tr.len());
    }
}

#[cfg(test)]
mod link_tests {
    use super::*;
    use super::tests::{atlas, theme_with};
    use crate::model::{Chat, Delivery, Message};
    use symbian_ui::BitmapFont;

    fn msg(text: &str, outgoing: bool) -> Message {
        Message {
            id: 1,
            text: alloc::string::String::from(text),
            outgoing,
            time: alloc::string::String::from("12:00"),
            state: Delivery::Read,
            media: None,
        }
    }

    fn chat_of(texts: &[&str]) -> Chat {
        let mut c = Chat::default();
        c.name = alloc::string::String::from("t");
        c.messages = texts.iter().map(|t| msg(t, false)).collect();
        c
    }

    const SCREEN: Rect = Rect { x0: 0, y0: 0, x1: 320, y1: 240 };

    /// A conversation with its layout built and the cursor parked on message `at`.
    ///
    /// The parking is the point. A fresh conversation opens at the *newest* message — which is what
    /// a chat client must do and what `ensure_layout` does on its first build — so a test that
    /// assumed index 0 would be testing the wrong row without ever saying so. The first version of
    /// these tests did exactly that and failed with a cursor one message from where it looked.
    fn conv_on(chat: &Chat, t: &Theme<'_>, at: usize) -> Conversation {
        let mut conv = Conversation::new(0);
        conv.ensure_layout(chat, t, SCREEN);
        let tr = conv.transcript.as_ref().unwrap();
        conv.state.select(at, tr, SCREEN.height());
        conv.link = None;
        // And in the transcript: the screen opens with the *composer* focused, so a test that did
        // not say otherwise would be sending its arrow keys to a text field.
        conv.focus = Focus::Transcript;
        conv
    }

    fn press(conv: &mut Conversation, chat: &Chat, t: &Theme<'_>, k: Key) -> ConvAction {
        conv.handle_key(KeyEvent::new(k), chat, t, SCREEN).1
    }

    /// The cursor as a pair: which message, and which link inside it.
    fn at(conv: &Conversation) -> (usize, Option<usize>) {
        (conv.state.selected, conv.link)
    }

    #[test]
    fn down_walks_message_then_its_links_then_the_next_message() {
        // The sequence the whole feature is: a message is a stop in its own right, and its links
        // are the stops after it.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let chat = chat_of(&["veja https://um.com e https://dois.com", "sem link aqui"]);
        let mut conv = conv_on(&chat, &t, 0);

        assert_eq!(at(&conv), (0, None), "starts on the message, not on a link");
        press(&mut conv, &chat, &t, Key::Down);
        assert_eq!(at(&conv), (0, Some(0)));
        press(&mut conv, &chat, &t, Key::Down);
        assert_eq!(at(&conv), (0, Some(1)));
        press(&mut conv, &chat, &t, Key::Down);
        assert_eq!(at(&conv), (1, None), "off the last link moves to the next message");
    }

    #[test]
    fn up_walks_the_same_sequence_backwards() {
        // The property that makes the model a sequence rather than two behaviours: arriving at a
        // message from below lands on its LAST link, so every stop Down visited, Up visits too.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let chat = chat_of(&["veja https://um.com e https://dois.com", "sem link aqui"]);
        let mut conv = conv_on(&chat, &t, 0);

        let mut forward = alloc::vec![at(&conv)];
        for _ in 0..3 {
            press(&mut conv, &chat, &t, Key::Down);
            forward.push(at(&conv));
        }
        let mut backward = alloc::vec![at(&conv)];
        for _ in 0..3 {
            press(&mut conv, &chat, &t, Key::Up);
            backward.push(at(&conv));
        }
        backward.reverse();
        assert_eq!(forward, backward, "the walk must be the same stops in reverse");
    }

    #[test]
    fn a_message_without_links_navigates_exactly_as_before() {
        // The regression that matters most: this feature must not make the common case worse. Two
        // plain messages, and the cursor never acquires a link at any point.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let chat = chat_of(&["primeira", "segunda", "terceira"]);
        let mut conv = conv_on(&chat, &t, 0);

        for _ in 0..2 {
            press(&mut conv, &chat, &t, Key::Down);
        }
        assert_eq!(at(&conv), (2, None));
        press(&mut conv, &chat, &t, Key::Up);
        assert_eq!(at(&conv), (1, None), "and Up never lands on a link that is not there");
    }

    #[test]
    fn select_on_a_link_opens_it_and_says_which() {
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let chat = chat_of(&["veja https://exemplo.com/a."]);
        let mut conv = conv_on(&chat, &t, 0);

        // On the message itself, Select is not a link press.
        assert_eq!(press(&mut conv, &chat, &t, Key::Select), ConvAction::None);
        press(&mut conv, &chat, &t, Key::Down);
        match press(&mut conv, &chat, &t, Key::Select) {
            ConvAction::OpenLink(u) => assert_eq!(u, "https://exemplo.com/a", "the full stop is not part of it"),
            other => panic!("expected OpenLink, got {other:?}"),
        }
    }

    #[test]
    fn a_focused_link_wins_over_the_message_media() {
        // A message can have both. The cursor is visibly on the link, so opening the attachment
        // instead would be the screen doing one thing while showing another.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let mut chat = chat_of(&["olha https://exemplo.com"]);
        chat.messages[0].media = Some(crate::model::Media::File {
            id: 1,
            access_hash: 1,
            file_reference: alloc::vec::Vec::new(),
            dc_id: 1,
            filename: alloc::string::String::from("a.pdf"),
            size: 10,
        });
        let mut conv = conv_on(&chat, &t, 0);

        // On the message: the media.
        assert!(matches!(press(&mut conv, &chat, &t, Key::Select), ConvAction::OpenMedia(0)));
        // On the link: the link.
        press(&mut conv, &chat, &t, Key::Down);
        assert!(matches!(press(&mut conv, &chat, &t, Key::Select), ConvAction::OpenLink(_)));
    }

    #[test]
    fn ctrl_c_copies_the_highlighted_message() {
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let chat = chat_of(&["primeira", "segunda"]);
        let mut conv = conv_on(&chat, &t, 1);
        match press(&mut conv, &chat, &t, Key::Ctrl('c')) {
            ConvAction::Copy(text) => assert_eq!(text, "segunda"),
            other => panic!("expected the message text, got {other:?}"),
        }
        // And it says so, because a copy is invisible otherwise — nothing on the screen changes.
        assert_eq!(conv.note.as_deref(), Some("mensagem copiada"));
    }

    #[test]
    fn ctrl_c_on_a_focused_link_copies_only_the_link() {
        // The same rule Select follows: the cursor is visibly on the link, so the link is what the
        // user meant. Copying the whole message would be the screen doing one thing while showing
        // another.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let chat = chat_of(&["olha https://exemplo.com"]);
        let mut conv = conv_on(&chat, &t, 0);
        press(&mut conv, &chat, &t, Key::Down);
        match press(&mut conv, &chat, &t, Key::Ctrl('c')) {
            ConvAction::Copy(text) => assert_eq!(text, "https://exemplo.com"),
            other => panic!("expected the link, got {other:?}"),
        }
    }

    #[test]
    fn ctrl_c_on_a_message_with_no_text_copies_nothing_and_says_so() {
        // A photo with no caption. Claiming "copiado" over an empty clipboard is worse than
        // saying nothing happened: the user pastes an hour later and gets whatever was there.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let chat = chat_of(&[""]);
        let mut conv = conv_on(&chat, &t, 0);
        assert!(matches!(press(&mut conv, &chat, &t, Key::Ctrl('c')), ConvAction::None));
        assert_eq!(conv.note.as_deref(), Some("nada para copiar"));
    }

    #[test]
    fn ctrl_c_in_the_composer_belongs_to_the_field_not_the_transcript() {
        // The field answers first and copies its own selection. If this screen took the key
        // instead, copying inside the composer would silently copy a message from the transcript.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let chat = chat_of(&["uma mensagem"]);
        let mut conv = conv_on(&chat, &t, 0);
        conv.focus = Focus::Composer;
        assert!(matches!(press(&mut conv, &chat, &t, Key::Ctrl('c')), ConvAction::None));
        assert_eq!(conv.note, None, "no message was copied");
    }

    #[test]
    fn walking_links_does_not_ask_for_older_messages() {
        // Up at the top of the transcript means "fetch the page above". With a link cursor that
        // branch has to wait its turn, or the first Up inside a link spends a network request.
        let data = atlas();
        let f = BitmapFont::new(&data).unwrap();
        let t = theme_with(&f);
        let chat = chat_of(&["https://um.com e https://dois.com"]);
        let mut conv = conv_on(&chat, &t, 0);

        press(&mut conv, &chat, &t, Key::Down);
        press(&mut conv, &chat, &t, Key::Down);
        assert_eq!(at(&conv), (0, Some(1)));
        assert_eq!(press(&mut conv, &chat, &t, Key::Up), ConvAction::None, "still walking links");
        assert_eq!(at(&conv), (0, Some(0)));
        // Only once the cursor is off the links does the top-of-transcript branch fire.
        assert_eq!(press(&mut conv, &chat, &t, Key::Up), ConvAction::None);
        assert_eq!(press(&mut conv, &chat, &t, Key::Up), ConvAction::LoadMore);
    }
}
