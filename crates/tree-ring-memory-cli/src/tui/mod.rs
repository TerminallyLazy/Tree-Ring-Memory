mod actions;
pub mod app;
mod event;
mod input;
mod model;
mod render;
mod rings;
mod store_watch;
mod stream;
mod theme;

use std::path::PathBuf;
use std::time::Duration;

use ratatui::backend::Backend;
use ratatui::crossterm::event as terminal_event;
use ratatui::Terminal;

use app::{App, AppMode};

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
                let previous_mode = app.mode;
                app.handle_key(key)?;
                clear_on_mode_change(terminal, previous_mode, app.mode)
                    .map_err(|err| err.to_string())?;
            }
        }
        app.tick()?;
    }
    Ok(())
}

fn clear_on_mode_change<B: Backend>(
    terminal: &mut Terminal<B>,
    previous_mode: AppMode,
    current_mode: AppMode,
) -> Result<(), B::Error> {
    if previous_mode != current_mode {
        terminal.clear()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::text::Text;

    use super::*;

    #[test]
    fn mode_change_clears_the_terminal_for_a_full_redraw() {
        let backend = TestBackend::new(12, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| frame.render_widget(Text::raw("stale frame"), frame.area()))
            .unwrap();

        clear_on_mode_change(&mut terminal, AppMode::Default, AppMode::Search).unwrap();

        assert!(!terminal.backend().to_string().contains("stale frame"));
    }

    #[test]
    fn unchanged_mode_preserves_the_current_terminal() {
        let backend = TestBackend::new(12, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| frame.render_widget(Text::raw("current"), frame.area()))
            .unwrap();

        clear_on_mode_change(&mut terminal, AppMode::Search, AppMode::Search).unwrap();

        assert!(terminal.backend().to_string().contains("current"));
    }
}
