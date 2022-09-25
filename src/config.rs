use chrono::{DateTime, NaiveTime, Timelike, Utc};
use directories::ProjectDirs;
use eyre::{Context, Result};
use ron;
use serde::{Deserialize, Deserializer};
use std::path::PathBuf;
use tui::style::Color;

// workaround to get rid of 'Some(bool)' in ron files
fn deserialize_option_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: bool = Deserialize::deserialize(deserializer)?;
    Ok(Some(value))
}

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub colors: Colors,
    pub tracks: Vec<TimeTrack>,
}

#[derive(Debug, Deserialize)]
pub struct Colors {
    #[serde(default = "default_base_color")]
    pub base: Color,
    #[serde(default = "default_fill_fg_color")]
    pub fill_fg: Color,
    #[serde(default = "default_clock_color")]
    pub clock: Color,
    #[serde(default = "default_title_color")]
    pub title: Color,
}

#[inline(always)]
fn default_base_color() -> Color {
    Color::DarkGray
}
#[inline(always)]
fn default_fill_fg_color() -> Color {
    Color::Black
}
#[inline(always)]
fn default_title_color() -> Color {
    Color::Reset
}
#[inline(always)]
fn default_clock_color() -> Color {
    Color::Reset
}

impl Default for Colors {
    fn default() -> Self {
        Colors {
            base: default_base_color(),
            fill_fg: default_fill_fg_color(),
            title: default_title_color(),
            clock: default_clock_color(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct TimeLabel {
    #[serde(default)]
    pub seconds: bool,
    #[serde(default)]
    pub blink: bool,
    #[serde(default)]
    pub fill: bool,
    #[serde(default)]
    pub use_range_color: bool,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct TimeRange {
    pub start: (u16, u16),
    pub end: (u16, u16),
    pub color: Color,
    #[serde(default, deserialize_with = "deserialize_option_bool")]
    pub fill: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_option_bool")]
    pub use_range_color: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_option_bool")]
    pub blink: Option<bool>,
}

impl TimeRange {
    pub fn new(start: (u16, u16), end: (u16, u16), color: Color) -> Self {
        TimeRange { start, end, color, fill: None, use_range_color: None, blink: None }
    }
}

impl Default for TimeRange {
    fn default() -> Self {
        Self {
            start: (0, 0),
            end: (24, 0),
            color: Color::DarkGray,
            fill: None,
            use_range_color: None,
            blink: None,
        }
    }
}
impl TimeRange {
    pub fn start(mut self, start: (u16, u16)) -> Self {
        self.start = start;
        self
    }
    pub fn end(mut self, end: (u16, u16)) -> Self {
        self.end = end;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

#[derive(Debug, Deserialize)]
pub struct TimeTrack {
    pub name: String,
    pub shortname: String,
    pub offset: (i16, i16),
    #[serde(default)]
    pub show_badge: bool,
    #[serde(default)]
    pub time_label: TimeLabel,
    #[serde(default)]
    pub ranges: Vec<TimeRange>,
}

impl TimeTrack {
    #[inline]
    pub fn offset(&self) -> chrono::Duration {
        chrono::Duration::hours(self.offset.0 as i64)
            + chrono::Duration::minutes((self.offset.1.abs() * self.offset.0.signum()) as i64)
    }

    #[inline]
    pub fn local_time(&self, now: DateTime<Utc>) -> (u16, u16) {
        let local: NaiveTime = (now + self.offset()).time();
        (local.hour() as u16, local.minute() as u16)
    }
}

#[inline]
fn read_config(path: PathBuf) -> Result<Config> {
    let s = std::fs::read_to_string(path.clone())
        .wrap_err_with(|| format!("Failed to read config file {:?}", path))?;
    ron::from_str(s.as_str())
        .map_err(|e| {
            eyre::eyre!(format!(
                "file {:?}\n{:?} at line:{} col:{}",
                path,
                e.code.to_string(),
                e.position.line,
                e.position.col
            ))
        })
        .wrap_err("Failed to parse config file")
}

#[inline]
fn validate_tracks(mut config: Config) -> Result<Config> {
    if config.tracks.is_empty() {
        eyre::bail!(
            "No tracks defined in the config, which implies it's an empty file.
            \nDefine some tracks, specify another config file, or check the github repo for pre-existing configurations"
        );
    }
    for track in &mut config.tracks {
        match (track.name.len(), track.shortname.len()) {
            (0, 0) => eyre::bail!(format!(
                "Unspecified title\n{:#?}\nspecify at least one of the fields ['name', 'shortname'] for this track",
                track
            )),
            (nl, sl) if nl > u16::MAX.into() || sl > u16::MAX.into() => {
                let msg = format!("Name is way too long\n{:#?}\nrename it to fit at most {} characters, 
                    although the real limit is `terminal line length` - 13",
                        track,
                        u16::MAX);
                eyre::bail!(msg);
            },
            // just clone it to simplify main loop computations
            (0, _) => track.name = track.shortname.clone(),
            (_, 0) => track.shortname = track.name.clone(),
            _ => ()
        }

        if track.offset.0.abs() > 23 || track.offset.1.abs() > 59 {
            eyre::bail!(format!(
                "UTC offset is not within the valid range\n{:#?}\nvalid value ranges for 'offset' is -23..23 for hour and -59..59 for minute",
                track));
        }

        if !track.ranges.is_empty() {
            for range in track.ranges.iter() {
                let start = range.start;
                let end = range.end;
                if start.0 > 23 || end.0 > 24 || start.1 > 59 || end.1 > 59 {
                    eyre::bail!(format!(
                "Time is out of range\n{:#?}\nvalid value ranges for 'range.start' and 'range.end' is 0..24 for hour and 0..59 for minute",
                track));
                }
                if start.0 * 100 + start.1 > 2400 {
                    eyre::bail!(format!("Wrong 'start' time \n{:#?}", track));
                }
                if end.0 * 100 + end.1 > 2400 {
                    eyre::bail!(format!("Wrong 'end' time \n{:#?}", track));
                }
                if start.0 * 100 + start.1 >= end.0 * 100 + end.1 {
                    eyre::bail!(format!("Wrong time ordering\n{:#?}\n'start' and 'end' time should be chronologically ordered such that 'start' << 'end'.", track));
                }
            }
            track
                .ranges
                .sort_by(|a, b| (a.start.0 * 100 + a.start.1).cmp(&(b.start.0 * 100 + b.start.1)));
            for i in 1..track.ranges.len() {
                let end = track.ranges[i - 1].end;
                let start = track.ranges[i].start;
                if start.0 * 100 + start.1 < end.0 * 100 + end.1 {
                    eyre::bail!(format!(
                    "Time range overlap\n{:#?}\nRanges should be chronologically exclusive, but these two overlaps \n{:?}\n{:?}",
                    track,
                    track.ranges[i - 1],
                    track.ranges[i]
                ));
                }
            }
        }
    }
    Ok(config)
}

pub fn load_config(path: PathBuf) -> Result<Config> {
    read_config(path).and_then(validate_tracks)
}

pub fn find_config(fname: &str) -> Result<PathBuf> {
    if fname.contains(".ron") {
        PathBuf::from(fname).canonicalize().wrap_err("failed to locate config file")
    } else {
        // search for <name>.ron in ./, ~/<CONFIG_DIRS>/mvtime/
        let mut paths = vec![];
        // ./<name>.ron
        let mut path = std::env::current_dir()?;
        path.push(fname);
        path.set_extension("ron");
        paths.push(path);

        // <Lin, Win, Mac specific config dir>/<name>.ron
        if let Some(prj) = ProjectDirs::from("com", "github 2davy", "mvtime") {
            let mut path = PathBuf::from(prj.config_dir());
            path.push(fname);
            path.set_extension("ron");
            paths.push(path);
        }
        paths
            .clone()
            .into_iter()
            .find(|p| p.exists())
            .ok_or_else(|| eyre::eyre!("{}.ron not found in any of the paths {:#?}", fname, paths))
    }
}
