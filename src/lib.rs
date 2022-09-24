use chrono::{self, DateTime, Timelike, Utc};
use config::{Colors, Config, TimeRange};
use eyre::{Result, WrapErr};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::Color,
    terminal::CompletedFrame,
    Terminal,
};
use ui::LineAux;
pub mod config;
pub mod ui;

pub const MINUTES_PER_DAY: u16 = to_minutes((24, 00));

#[inline]
pub const fn to_minutes((hour, minute): (u16, u16)) -> u16 {
    hour * 60 + minute
}

#[inline]
pub fn to_hour_minute(minutes: u16) -> (u16, u16) {
    (minutes / 60, minutes % 60)
}

// map point to time given time axes width in units
#[inline]
pub fn point_to_time(idx: u16, width: u16) -> u16 {
    debug_assert_ne!(width, 0, "Hoy! 0 width, init?");
    let ratio = f64::from(MINUTES_PER_DAY) / f64::from(width);
    (f64::from(idx) * ratio).round() as u16
}

// map time to time range
#[inline]
pub fn time_to_range<R: AsRef<[TimeRange]>>(time: u16, ranges: R) -> Option<usize> {
    debug_assert_ne!(ranges.as_ref().len(), 0, "Hoy! Empty ranges.");
    ranges.as_ref().iter().position(|r| (to_minutes(r.start)..to_minutes(r.end)).contains(&time))
}

// given sequence of time ranges, fill in-between gaps to cover full range of (0,0) - (24,0).
// assumes provided ranges are valid:
// - start and end is within the interval [00:00, 24:00]
// - start << end
// - ranges are non-overlapping and chronologically ordered
#[inline]
pub fn fill_gaps(ranges: &mut Vec<TimeRange>, colors: &Colors) {
    let mut end = 0;

    for mut i in 0..ranges.len() {
        if to_minutes(ranges[i].start) > end {
            ranges.insert(
                i,
                TimeRange::default()
                    .start(to_hour_minute(end))
                    .end(ranges[i].start)
                    .color(colors.base),
            );
            i += 1;
        }
        end = to_minutes(ranges[i].end);
    }

    if ranges.is_empty() || end < MINUTES_PER_DAY {
        ranges
            .push(TimeRange::default().start(to_hour_minute(end)).end((24, 00)).color(colors.base));
    }
}

pub struct App {
    config: Config,
    min_title_width: u16,
    max_title_width: u16,
    min_width: u16,
    min_height: u16,
    lines: Vec<LineAux>,
    visible_lines: usize,
    seconds: u16,
    renderable: bool,
}

impl App {
    pub fn new(mut config: Config) -> Self {
        // Tracks preprocessing and layout initialization

        // min/max title and clock columns width
        // max: badge(1) _ longest(name)
        // min: badge(1) _ longest(shortname)
        // clock: 10 if any of the `seconds` is set to true, 7 otherwise
        let mut max_title = u16::MIN;
        let mut min_title = u16::MIN;
        let mut min_clock = 7;
        for track in &mut config.tracks {
            max_title = u16::max(max_title, track.name.len() as u16 + 2);
            min_title = u16::max(min_title, track.shortname.len() as u16 + 2);
            min_clock = u16::max(min_clock, track.time_label.seconds as u16 * 10);
            // fill time range gaps, so ranges cover whole day
            fill_gaps(&mut track.ranges, &config.colors);
        }
        // minimum displayable screen size
        // width: title _ notch(1) bar(1) clock(7/10) bar(1) notch(1) __
        let min_width = min_title + 1 + 1 + 1 + min_clock + 1 + 1 + 2;
        // height: margin(1) line(1) margin(1)
        let min_height = 3;

        // sort tracks by utc offset
        config.tracks.sort_by_key(|a| a.offset());

        Self {
            lines: vec![LineAux::default(); config.tracks.len()],
            config,
            min_title_width: min_title,
            max_title_width: max_title,
            min_width,
            min_height,
            visible_lines: 0,
            seconds: 0,
            renderable: false,
        }
    }

    // Screen size related computations
    // executes on 'resize' event
    pub fn update_layout(&mut self, mut inner: Rect) {
        if inner.width < self.min_width || inner.height < self.min_height {
            self.renderable = false;
            return;
        }
        self.renderable = true;
        // add top(1), bottom(1), right(2) margins
        inner.y += 1;
        inner.height -= 2;
        inner.width -= 2;

        // title col width
        // if `screen width / max title len` > 4 => use 'name', otherwise use 'shortname'
        let wide_title = inner.width as f64 / self.max_title_width as f64 > 4.;
        let title_width =
            wide_title as u16 * self.max_title_width + !wide_title as u16 * self.min_title_width;

        // for clock column to divide chart area into two equal halves chart area width should be odd,
        // in case of even chart adjust right margin by 1
        let adj = f64::from(inner.width - 3 - title_width) / 2.;
        inner.width += (adj.fract() == 0.) as u16;

        // number of visible tracks is limited by screen height
        let nlines = usize::min(inner.height as usize, self.config.tracks.len());
        self.visible_lines = nlines;

        // column layout
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Length(1),           // badge
                    Constraint::Length(1),           // _
                    Constraint::Length(title_width), // title
                    Constraint::Length(1),           // _
                    Constraint::Min(0),              // chart area
                ]
                .as_ref(),
            )
            .split(inner);
        let badges = chunks[0];
        let titles = chunks[2];
        let charts = chunks[4];

        // clocks
        let mut clocks = charts;
        clocks.width = 7;
        clocks.x += ((charts.width as f64 - 7.0) / 2.0) as u16;
        debug_assert_eq!(
            clocks.left() - charts.left(),
            charts.right() - clocks.left() - 7,
            "chart should be divisible by clock column width into two equal halves"
        );

        // line rects
        for i in 0..nlines {
            let track = &self.config.tracks[i as usize];
            let aux = &mut self.lines[i];

            aux.title_text.clear();
            aux.title_text.push_str([&track.shortname, &track.name][wide_title as usize]);

            let mut line = inner;
            line.height = 1;
            line.y += i as u16;

            let chart = line.intersection(charts);

            // clock width:
            // _ hh : mm _
            // _ hh : mm : ss _
            let mut clock_rect = clocks.intersection(line);
            clock_rect.width = 7 + track.time_label.seconds as u16 * 3;

            aux.badge = badges.intersection(line);
            aux.title = titles.intersection(line);
            aux.chart = chart;
            aux.clock = clock_rect;
            aux.bars.0.y = line.y;
            aux.bars.1.x = clock_rect.right();
            aux.bars.1.y = line.y;
        }
    }

    // Time related computations
    // executes on 'tick' event, presumably once in a second
    pub fn tick(&mut self, now: DateTime<Utc>) {
        self.seconds = now.time().second() as u16;
        for i in 0..self.visible_lines {
            let track = &self.config.tracks[i];
            let aux = &mut self.lines[i];

            // get track-local time
            let (hour, minute) = track.local_time(now);
            aux.local_time.0 = hour;
            aux.local_time.1 = minute;

            // find current active range
            let minutes = to_minutes((hour, minute));
            let current_range_idx =
                time_to_range(minutes, &track.ranges).expect("shouldn't fail neva-eva");
            aux.current_range = current_range_idx;

            // compute bar widths and positions
            let left_width = aux.clock.left() - aux.chart.left();
            let ratio = f64::from(left_width) / f64::from(MINUTES_PER_DAY);
            let width = (f64::from(minutes) * ratio).round();
            let width = u16::max(width as u16, 1);
            aux.bars.0.width = width;
            aux.bars.0.x = aux.clock.left() - width;

            let right_width = aux.chart.right() - aux.clock.right();
            let ratio = f64::from(right_width) / f64::from(MINUTES_PER_DAY);
            let width = (f64::from(MINUTES_PER_DAY - minutes) * ratio).round();
            let width = u16::max(width as u16, 1);
            aux.bars.1.width = width;
        }
    }

    pub fn render<'a, B: Backend>(
        &mut self,
        terminal: &'a mut Terminal<B>,
    ) -> Result<CompletedFrame<'a>> {
        if !self.renderable {
            // screen size is not enough to display any meaningfull chart,
            // draw a blank screen
            return terminal
                .draw(|f| ui::fill(f, f.size(), Color::Reset))
                .wrap_err("Failed to draw a frame");
        }

        // compute new bar data
        self.tick(chrono::offset::Utc::now());

        // draw ui
        terminal
            .draw(|frame| {
                for (line, track) in self
                    .lines
                    .iter()
                    .take(self.visible_lines)
                    .zip(self.config.tracks.iter().take(self.visible_lines))
                {
                    if track.show_badge {
                        ui::render_badge(frame, line, track, &self.config.colors);
                    }
                    ui::render_title(frame, line, &self.config.colors);
                    ui::render_clock(frame, self.seconds, line, track, &self.config.colors);
                    ui::render_bars(frame, line, track);
                }
            })
            .wrap_err("Failed to draw a frame")
    }
}

#[cfg(test)]
mod tests {
    use tui::style::Color;

    use crate::*;

    // point_to_time

    #[test]
    fn begin_end_works() {
        let width = 15;
        assert_eq!(point_to_time(0, width), 0);
        assert_eq!(point_to_time(width, width), to_minutes((24, 00)));
        let width = 8;
        assert_eq!(point_to_time(0, width), 0);
        assert_eq!(point_to_time(width, width), to_minutes((24, 00)));
    }

    #[test]
    fn width_of_one() {
        let width = 1;
        assert_eq!(point_to_time(0, width), 0);
        assert_eq!(point_to_time(width, width), to_minutes((24, 00)));
    }

    #[test]
    fn middle_point_even_width() {
        let width = 8;
        assert_eq!(point_to_time(4, width), to_minutes((12, 0)));
    }

    #[test]
    fn test_all_in_range_one_to_one_ratio() {
        let width = to_minutes((24, 00));
        for i in 0..width {
            assert_eq!(point_to_time(i, width), i);
        }
    }

    #[test]
    fn test_all_in_range_one_to_two_ratio() {
        let width = to_minutes((12, 0));
        for i in 0..width {
            assert_eq!(point_to_time(i, width), i * 2);
        }
    }

    #[test]
    fn test_all_in_range_one_to_three_ratio() {
        let width = (f64::from(to_minutes((24, 00))) / 3.).floor();
        let width = width as u16;
        for i in 0..width {
            assert_eq!(point_to_time(i, width), i * 3);
        }
    }

    // time_to_range

    #[test]
    fn simple_range_inclusive() {
        let ranges = vec![TimeRange::new((0, 0), (24, 0), Color::Reset)];
        assert_eq!(time_to_range(0, &ranges[..]), Some(0));
        assert_eq!(time_to_range(to_minutes((23, 59)), &ranges[..]), Some(0));
    }

    #[test]
    fn simple_range_out_of() {
        let ranges = vec![TimeRange::new((0, 0), (12, 0), Color::Reset)];
        assert_eq!(time_to_range(to_minutes((12, 0)), &ranges[..]), None);
    }

    #[test]
    fn simple_range2() {
        let ranges = vec![
            TimeRange::new((0, 0), (12, 0), Color::Reset),
            TimeRange::new((12, 0), (24, 0), Color::Reset),
        ];
        assert_eq!(time_to_range(to_minutes((12, 00)), &ranges[..]), Some(1));
    }

    #[test]
    fn ranges_exclusive() {
        let ranges = vec![
            TimeRange::new((0, 0), (12, 0), Color::Reset),
            TimeRange::new((12, 0), (24, 0), Color::Reset),
        ];
        assert_eq!(time_to_range(to_minutes((11, 59)), &ranges[..]), Some(0));
        assert_eq!(time_to_range(to_minutes((12, 00)), &ranges[..]), Some(1));
        assert_eq!(time_to_range(to_minutes((24, 00)), &ranges[..]), None);
    }

    #[test]
    fn ranges_non_contiguous() {
        let ranges = vec![
            TimeRange::new((0, 0), (1, 0), Color::Reset),
            TimeRange::new((5, 0), (6, 0), Color::Reset),
            TimeRange::new((12, 0), (24, 0), Color::Reset),
        ];
        assert_eq!(time_to_range(to_minutes((0, 59)), &ranges[..]), Some(0));
        assert_eq!(time_to_range(to_minutes((5, 0)), &ranges[..]), Some(1));
        assert_eq!(time_to_range(to_minutes((12, 00)), &ranges[..]), Some(2));
        assert_eq!(time_to_range(to_minutes((24, 00)), &ranges[..]), None);
    }

    // fill_gaps

    #[test]
    fn fill_empty_ranges() {
        let mut r = vec![];
        fill_gaps(&mut r, &Default::default());
        assert_eq!(r.len(), 1);
        assert_eq!(to_minutes(r[0].start), 0);
        assert_eq!(to_minutes(r[0].end), to_minutes((24, 00)));
    }

    #[test]
    fn fill_full_range_shouldnt_do_any() {
        let mut r = vec![TimeRange::new((0, 0), (24, 0), Color::DarkGray)];
        fill_gaps(&mut r, &Default::default());
        assert_eq!(r.len(), 1);
        assert_eq!(to_minutes(r[0].start), 0);
        assert_eq!(to_minutes(r[0].end), to_minutes((24, 00)));
    }
    #[test]
    fn fill_start() {
        let mut r = vec![TimeRange::new((12, 0), (24, 0), Color::DarkGray)];
        fill_gaps(&mut r, &Default::default());
        assert_eq!(r.len(), 2);
        assert_eq!(to_minutes(r[0].start), 0);
        assert_eq!(to_minutes(r[0].end), to_minutes((12, 00)));
        assert_eq!(to_minutes(r[1].start), to_minutes((12, 00)));
        assert_eq!(to_minutes(r[1].end), to_minutes((24, 00)));
    }

    #[test]
    fn fill_end() {
        let mut r = vec![TimeRange::new((0, 0), (12, 0), Color::DarkGray)];
        fill_gaps(&mut r, &Default::default());
        assert_eq!(r.len(), 2);
        assert_eq!(to_minutes(r[0].start), 0);
        assert_eq!(to_minutes(r[0].end), to_minutes((12, 00)));
        assert_eq!(to_minutes(r[1].start), to_minutes((12, 00)));
        assert_eq!(to_minutes(r[1].end), to_minutes((24, 00)));
    }

    #[test]
    fn fill_in_between() {
        let mut r = vec![
            TimeRange::new((0, 0), (10, 0), Color::DarkGray),
            TimeRange::new((18, 0), (24, 0), Color::DarkGray),
        ];
        fill_gaps(&mut r, &Default::default());
        assert_eq!(r.len(), 3);
        assert_eq!(to_minutes(r[0].start), 0);
        assert_eq!(to_minutes(r[0].end), to_minutes((10, 00)));
        assert_eq!(to_minutes(r[1].start), to_minutes((10, 0)));
        assert_eq!(to_minutes(r[1].end), to_minutes((18, 00)));
        assert_eq!(to_minutes(r[2].start), to_minutes((18, 0)));
        assert_eq!(to_minutes(r[2].end), to_minutes((24, 00)));
    }

    #[test]
    fn fill_end_contiguous() {
        let mut r = vec![
            TimeRange::new((0, 0), (12, 0), Color::DarkGray),
            TimeRange::new((12, 0), (18, 0), Color::DarkGray),
        ];
        fill_gaps(&mut r, &Default::default());
        assert_eq!(r.len(), 3);
        assert_eq!(to_minutes(r[0].start), 0);
        assert_eq!(to_minutes(r[0].end), to_minutes((12, 00)));
        assert_eq!(to_minutes(r[1].start), to_minutes((12, 00)));
        assert_eq!(to_minutes(r[1].end), to_minutes((18, 00)));
        assert_eq!(to_minutes(r[2].start), to_minutes((18, 00)));
        assert_eq!(to_minutes(r[2].end), to_minutes((24, 00)));
    }
}
