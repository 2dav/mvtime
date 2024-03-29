# Multiverse time
## About
Multi-timezone wall clock inspired by [everytimezone](https://everytimezone.com), for terminals.
<p align="center">
  <img alt="Example of mvtime configured for asset markets" src="img/head.png">
</p>

## Usage
> mvtime [options] [config]

where `config` is the path to the [configuration](#configuration) file, and options are

    -h, --help    Print help information
    -l, --live    Run app in live mode

see [examples](#examples) for some of the existing configurations

## Build
> cargo build --release

move `target/release/mvtime` binary to any location on your `PATH`

## Examples
[**EveryTimeZone**](timezones.ron) 

> mvtime -l timezones.ron 

<p align="center">
  <img alt="Example of mvtime configured to match everytimezone.com" src="img/tz.png">
</p>

This example colors different times of the day in common timezones on the planet Earth. 

`Moscow/Russia` row in the middle is configured to be the 'local time' upon which other bars are positioned,\
so it is 7:59PM of yesterday(relative to local) in the US, meanwhile in Russia it is 4:59AM of today,
and New Zealand is already passed this day for a half.

[**Asset exchanges**](markets.ron)
> mvtime -l markets.ron 

<p align="center">
  <img alt="Example of mvtime configured for asset markets" src="img/mkt.png">
</p>

The asset exchanges operates in the different regimes during the day, they are common to all exchanges,
but vary in duration and continuity, this example uses colors to encode these features:
- white areas denote 'morning trading session', the specific period at the beginning of the day
- yellow areas - 'main trading session'
- and the blue areas are 'evening trading session'

## Configuration

Configuration file is the list of `time tracks` in the [RON](https://docs.rs/ron/latest/ron) file format.

`(tracks: [])`- minimal valid config

*Config file is reloaded automatically when changed.*

### Tracks
```
(name: "",
 shortname:  "",
 offset:     (int, int),
 show_badge: bool,
 time_label: (blink: bool, 
              seconds: bool, 
              fill: bool, 
              use_range_color: bool),
 ranges:    [(start:(int, int), end:(int, int), color: Color, fill:bool, blink:bool)])
```
- **name** - track title
- ***shortname** - alternative track title in compact mode
- **offset** - UTC offset in 24-hour format `(HH,MM) (-23..23, -59..59)`
- ***show_badge** - whether to show 'badge' to the left of the title, `false` by default
- ***time_label** - time label options
	- ***blink** - controls blinking of `:`, `false` by default
	- ***seconds** - show seconds, `false` by default
	- ***fill** - use background color, `transparent` by default
	- ***use_range_color** - set active range [color](#colors) as a background, `false` by default

### Ranges
```
ranges: [(start:(9, 30), end:(12, 00)),
        (start:(13, 00), end:(16, 00), color: Yellow, fill:true, blink:true)]
```
- **ranges** - list of time ranges
	- **start**  - start of the range in 24-hour format `(HH,MM) (0..24, 0..59)`
	- **end** - end of the range `(HH,MM) (0..24, 0..59)`
	- ***color** - range [color](#colors)
	- ***fill** - temporary overrides `time_label.fill` when range is active
	- ***blink** - temporary overrides `time_label.blink` when range is active

### Colors
list of possible color values
```
Reset,
Black,
Red,
Green,
Yellow,
Blue,
Magenta,
Cyan,
Gray,
DarkGray,
LightRed,
LightGreen,
LightYellow,
LightBlue,
LightMagenta,
LightCyan,
White,
Rgb(u8, u8, u8)
```

*If you found some interesting usage, do miss some features, or just wants to share your configuration,
feel free to fill the issue.*
