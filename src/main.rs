use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use eyre::{Result, WrapErr};
use mvtime::{config, App};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    io::{self, Stdout},
    path::PathBuf,
    sync::mpsc::Receiver,
    time::{Duration, SystemTime},
};
use tui::{backend::CrosstermBackend, layout::Rect, Terminal};

const TICK_RATE: i64 = 1000;

fn is_exit_key(key: KeyEvent) -> bool {
    const CTRL_C: KeyEvent = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    key.code.eq(&KeyCode::Char('q')) || key.code.eq(&KeyCode::Esc) || key.eq(&CTRL_C)
}

fn poll(ms: u64) -> Result<Option<Event>> {
    if event::poll(Duration::from_millis(ms)).wrap_err("Failed to poll for new terminal events")? {
        let e = event::read().wrap_err("Failed to read new terminal events")?;
        Ok(Some(e))
    } else {
        Ok(None)
    }
}

fn init(one_time: bool) -> Result<Terminal<CrosstermBackend<Stdout>>> {
    let mut stdout = io::stdout();
    enable_raw_mode().wrap_err("Switching to raw terminal mode failed")?;
    if !one_time {
        execute!(stdout, EnterAlternateScreen).wrap_err("Alternate screen switching failed")?;
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).wrap_err("Terminal backend initialization failed")?;
    terminal.clear()?;
    terminal.hide_cursor()?;
    Ok(terminal)
}

fn start_watcher(
    path: PathBuf,
) -> Result<(notify::INotifyWatcher, Receiver<Result<notify::Event, notify::Error>>)> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    watcher.watch(path.as_ref(), RecursiveMode::NonRecursive)?;
    Ok((watcher, rx))
}

fn should_reload(rx: &Receiver<Result<notify::Event, notify::Error>>) -> bool {
    if let Ok(e) = rx.try_recv() {
        e.is_ok() && e.unwrap().kind.is_modify()
    } else {
        false
    }
}

fn finalize(mut terminal: Terminal<CrosstermBackend<Stdout>>, one_time: bool) -> Result<()> {
    disable_raw_mode()?;
    if !one_time {
        execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    }
    terminal.show_cursor()?;
    Ok(())
}

fn main() -> Result<()> {
    #[cfg(not(debug_assertions))]
    simple_eyre::install()?;

    let args = [clap::Arg::new("mode")
                .short('l')
                .long("live")
                .takes_value(false)
                .help("Run app in live mode"),
        clap::Arg::new("config")
        .max_occurrences(1)
        .default_value("default")
        .help("Config file to use. Might be specified as path, or a file name without '.ron' extension, \
in this case it will be searched in './' and '<OS_CONFIGS_LOCATION>/mvtime'")];
    let matches =
        clap::Command::new("mvtime").about("Multiverse CLI time tracker").args(args).get_matches();

    let one_time = !matches.is_present("mode");
    let config = config::find_config(matches.value_of("config").unwrap())
        .wrap_err("Can't find a config file")?;

    // Load/Parse config file
    let tracks_cfg = config::load_config(config.clone())?;

    // start config file change watcher
    let (_watcher, change_event) = start_watcher(config.clone())?;

    let mut terminal = init(one_time)?;
    let mut app = App::new(tracks_cfg);
    terminal.size().map(|rect| app.update_layout(rect))?;

    if one_time {
        app.render(&mut terminal)?;
        return finalize(terminal, true);
    }

    'main: loop {
        // render
        app.render(&mut terminal)?;

        // keyboard handling
        let mut dt = TICK_RATE;
        while dt > 0 {
            let ts = SystemTime::now();
            match poll(dt as u64)? {
                Some(Event::Key(key)) if is_exit_key(key) => break 'main,
                Some(Event::Resize(w, h)) => app.update_layout(Rect::new(0, 0, w, h)),
                _ => {}
            }
            dt -= ts.elapsed().wrap_err("Time have gone backwards somehow")?.as_millis() as i64;
        }

        // config reloading
        if should_reload(&change_event) {
            // stay on the current config if the new one is invalid
            if let Ok(cfg) = config::load_config(config.clone()) {
                app = App::new(cfg);
                terminal.size().map(|rect| app.update_layout(rect))?;
            }
        }
    }

    finalize(terminal, false)
}
