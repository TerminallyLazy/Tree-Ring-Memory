mod actions;
pub mod app;
mod event;
mod input;
mod model;
mod render;
mod rings;
mod store_watch;
mod stream;

use std::path::PathBuf;
use std::time::Duration;

use ratatui::crossterm::event as terminal_event;

use app::App;

pub fn run(root: PathBuf, event_stream: Option<PathBuf>, tick_ms: u64) -> Result<(), String> {
    let tick_rate = Duration::from_millis(tick_ms.clamp(50, 5_000));
    let mut app = App::new(root, event_stream)?;
    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &mut app, tick_rate);
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    tick_rate: Duration,
) -> Result<(), String> {
    while !app.should_quit {
        terminal
            .draw(|frame| render::render(frame, app))
            .map_err(|err| err.to_string())?;

        if terminal_event::poll(tick_rate).map_err(|err| err.to_string())? {
            if let terminal_event::Event::Key(key) =
                terminal_event::read().map_err(|err| err.to_string())?
            {
                app.handle_key(key)?;
            }
        }
        app.tick()?;
    }
    Ok(())
}
