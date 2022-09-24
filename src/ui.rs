use tui::{
    backend::Backend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    symbols,
    widgets::{Block, Widget},
    Frame,
};

use crate::{
    config::{Colors, TimeTrack},
    point_to_time, time_to_range,
};

const DEBUG_LAYOUT: bool = false;

#[inline(always)]
pub fn fill<B: Backend>(f: &mut Frame<B>, area: Rect, color: Color) {
    f.render_widget(Block::default().style(Style::default().bg(color)), area);
}

#[inline(always)]
pub fn debug_fill<B: Backend>(f: &mut Frame<B>, area: Rect, color: Color) {
    if DEBUG_LAYOUT {
        fill(f, area, color);
    }
}

// Aux track layout rects, point-range mappings, styles and texts.
// This struct is recomputed from track data on resize and updated on ticks.
#[derive(Debug, Default, Clone)]
pub struct LineAux {
    pub badge: Rect,
    pub title: Rect,
    pub chart: Rect,
    pub clock: Rect,
    pub bars: (Rect, Rect),
    pub current_range: usize,
    pub title_text: String,
    pub local_time: (u16, u16),
}

// fill one line with symbol
pub struct Glyph {
    style: Style,
    symbol: &'static str,
}

impl Glyph {
    fn new(symbol: &'static str, style: Style) -> Self {
        Self { style, symbol }
    }
}

impl Widget for Glyph {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let i_0 = buf.index_of(area.left(), area.top());
        let i_n = buf.index_of(area.right(), area.top());
        let cells = &mut buf.content[i_0..i_n];
        for cell in cells {
            cell.set_style(self.style).set_symbol(self.symbol);
        }
    }
}

// one line of text
struct TextLine<'a> {
    text: &'a str,
    style: Style,
}

impl<'a> TextLine<'a> {
    pub fn new(text: &'a str, style: Style) -> Self {
        Self { text, style }
    }
}

impl<'a> Widget for TextLine<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let n = self.text.len();
        let i_0 = buf.index_of(area.left(), area.top());
        let i_n = buf.index_of(area.left() + n as u16, area.top());
        let cells = &mut buf.content[i_0..i_n];
        for (symbol, cell) in
            cells.iter_mut().enumerate().map(|(i, cell)| (&self.text[i..=i], cell))
        {
            cell.set_style(self.style).set_symbol(symbol);
        }
    }
}

// the way to get tui's underlying terminal buffer
#[repr(transparent)]
struct Apply<F: Fn(&mut Buffer)>(F);
impl<F: Fn(&mut Buffer)> Widget for Apply<F> {
    fn render(self, _: Rect, buf: &mut Buffer) {
        self.0(buf);
    }
}

#[inline]
pub fn render_badge<B: Backend>(
    frame: &mut Frame<B>,
    line: &LineAux,
    track: &TimeTrack,
    colors: &Colors,
) {
    debug_fill(frame, line.badge, Color::Magenta);
    const THIN: &str = symbols::block::NINE_LEVELS.one_eighth;
    const THICK: &str = symbols::block::NINE_LEVELS.one_quarter;
    //const SYMBOL: &str = symbols::DOT;

    let range_color = track.ranges[line.current_range as usize].color;
    let symbol = if range_color == colors.base { THIN } else { THICK };

    frame.render_widget(Glyph::new(symbol, Style::default().fg(range_color)), line.badge);
}

#[inline]
pub fn render_title<B: Backend>(frame: &mut Frame<B>, line: &LineAux, colors: &Colors) {
    debug_fill(frame, line.title, Color::Cyan);
    frame.render_widget(
        TextLine::new(
            &line.title_text,
            Style::default().fg(colors.title).add_modifier(Modifier::BOLD),
        ),
        line.title,
    );
}

#[inline]
pub fn render_clock<B: Backend>(
    frame: &mut Frame<B>,
    seconds: u16,
    line: &LineAux,
    track: &TimeTrack,
    colors: &Colors,
) {
    debug_fill(frame, line.clock, Color::Blue);
    let label = &track.time_label;
    let (hour, minute) = line.local_time;
    let range = track.ranges[line.current_range as usize];
    let rc = range.color;
    let text = if label.seconds {
        format!(" {:02}:{:02}:{:02} ", hour, minute, seconds)
    } else {
        format!(" {:02}:{:02} ", hour, minute)
    };
    let fill = range.fill.unwrap_or(label.fill);
    let blink = range.blink.unwrap_or(label.blink);
    let use_range_color = range.use_range_color.unwrap_or(label.use_range_color);

    let (fg, bg) = match (use_range_color, fill) {
        (true, true) => (colors.fill_fg, rc),
        (true, false) => (rc, Color::Reset),
        (false, true) => (colors.fill_fg, colors.base),
        (false, false) => (colors.clock, Color::Reset),
    };
    let style = Style::default().add_modifier(Modifier::BOLD).fg(fg).bg(bg);
    frame.render_widget(TextLine::new(text.as_str(), style), line.clock);

    if blink {
        let style = style.add_modifier(Modifier::SLOW_BLINK);
        frame.render_widget(
            Apply(|buf| {
                buf.get_mut(line.clock.x + 3, line.clock.y).set_style(style);
                if label.seconds {
                    buf.get_mut(line.clock.x + 6, line.clock.y).set_style(style);
                }
            }),
            line.clock,
        );
    }
}

#[inline]
pub fn render_bars<B: Backend>(frame: &mut Frame<B>, line: &LineAux, track: &TimeTrack) {
    debug_fill(frame, line.chart, Color::Blue);
    const SYMBOL: &str = symbols::line::NORMAL.horizontal;
    const NOTCH: &str = symbols::line::THICK.horizontal;
    frame.render_widget(
        Apply(|buf| {
            let (lbar, rbar) = &line.bars;
            let y = lbar.y;
            let l0 = buf.index_of(lbar.left(), y);
            let ln = buf.index_of(lbar.right(), y);
            let r0 = buf.index_of(rbar.left(), y);
            let rn = buf.index_of(rbar.right(), y);
            let cells = (l0..ln).into_iter().chain((r0..rn).into_iter());
            let total_width = lbar.width + rbar.width;
            for (i, ci) in cells.enumerate() {
                let time = point_to_time(i as u16, total_width);
                let range_idx = time_to_range(time, &track.ranges).unwrap();
                let range = track.ranges[range_idx];
                let style = Style::default().fg(range.color);
                buf.content[ci].set_style(style).set_symbol(SYMBOL);
            }
            buf.content[l0].set_symbol(NOTCH).set_fg(track.ranges[0].color);
            buf.content[rn - 1]
                .set_symbol(NOTCH)
                .set_fg(track.ranges[track.ranges.len() - 1].color);
        }),
        frame.size(),
    );
}
