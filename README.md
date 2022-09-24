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

see [examples](#examples) for some existing configurations

## Build
> cargo build --release

move `target/release/mvtime` binary to any location on your `PATH`


## Examples
**EveryTimeZone**
> mvtime -l timezones.ron 

<p align="center">
  <img alt="Example of mvtime configured to match everytimezone.com" src="img/tz.png">
</p>

This example colors different times of the day throughout the planet earth. 

`Moscow/Russia` row in the middle is configured to be the 'local time' upon which other bars are positioned,\
so it is 7:59PM of yesterday(relative to local) in the US, meanwhile in Russia it is 4:59AM of today,
and New Zealand is already passed this day for a half.

**Global assets exchanges**
> mvtime -l markets.ron 

<p align="center">
  <img alt="Example of mvtime configured for asset markets" src="img/mkt.png">
</p>

Asset exchanges works in the different regimes throughout the day, these are common for all exchanges but differs in duration and continuity, this
example uses colors to code these:
- white regions denotes 'morning trading session', specific period at the start of the day
- yellow regions are 'main trading session'
- and blue regions are 'evening trading session'

## Configuration
