# Multiverse time
![global markets](img/mkt.png)

TUI multi timezone wall clock.

## Example configurations
**Earth time**

> cargo run timezones.ron 

![global timezones](img/tz.png)

This example colors different times of the day throughout the planet earth. 

`Moscow/Russia` row in the middle is configured to be the 'local time' upon which other bars are positioned,\
so it is 7:59PM of yesterday(relative to local) in the US, meanwhile in Russia it is 4:59AM of today,
and New Zealand is already passed this day for a half.

**Global assets exchanges**
> cargo run markets.ron 

![global markets](img/mkt.png)

Asset exchanges works in the different regimes throughout the day, these are common for all exchanges but differs in duration and continuity, this
example uses colors to code these:
- white regions denotes 'morning trading session', specific period at the start of the day
- yellow regions are 'main trading session'
- and blue regions are 'evening trading session'

