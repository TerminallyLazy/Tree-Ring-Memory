use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use super::app::{App, AppMode};
use super::input::command_help;
use super::rings::{ambient_tree_lines, exploded_ring_lines, ring_style};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    if area.width < 72 || area.height < 22 {
        render_compact(frame, area, app);
        return;
    }

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, vertical[0], app);
    render_body(frame, vertical[1], app);
    render_footer(frame, vertical[2], app);

    if let Some(pending) = &app.pending_action {
        render_confirmation(
            frame,
            centered_rect(70, 22, area),
            &pending.confirmation_prompt(),
        );
    }
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mode = match app.mode {
        AppMode::Default => "ambient",
        AppMode::Exploded => "exploded",
        AppMode::Command => "command",
        AppMode::Search => "search",
        AppMode::Stream => "stream",
        AppMode::Watch => "watch",
    };
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "TREE RING MEMORY",
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  mode:{mode}")),
        Span::raw(format!("  total:{}", app.dashboard.total)),
        Span::raw(format!("  private:{}", app.dashboard.sensitive_total)),
        Span::raw(format!("  status: {}", app.status)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, area);
}

fn render_body(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
        .split(columns[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(54), Constraint::Percentage(46)])
        .split(columns[1]);

    if app.mode == AppMode::Exploded {
        render_exploded(frame, left[0], app);
    } else {
        render_ambient(frame, left[0], app);
    }
    render_ring_hud(frame, left[1], app);
    render_results(frame, right[0], app);
    render_detail(frame, right[1], app);
}

fn render_ambient(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let paragraph = Paragraph::new(ambient_tree_lines(&app.dashboard, app.tick))
        .block(
            Block::default()
                .title("Ambient Rings")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_exploded(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let paragraph = Paragraph::new(exploded_ring_lines(&app.dashboard, app.selected_ring))
        .block(Block::default().title("/rings").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_ring_hud(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let items: Vec<ListItem<'_>> = app
        .dashboard
        .rings
        .iter()
        .enumerate()
        .map(|(index, stats)| {
            let selected = if index == app.selected_ring { ">" } else { " " };
            let line = Line::from(vec![
                Span::raw(selected),
                Span::styled(format!(" {:<10}", stats.ring), ring_style(stats)),
                Span::raw(format!(
                    " {:>4} avg {:.2}/{:.2} private {}",
                    stats.total,
                    stats.average_confidence,
                    stats.average_salience,
                    stats.sensitive_count
                )),
            ]);
            ListItem::new(line)
        })
        .collect();
    let list = List::new(items).block(Block::default().title("Rings").borders(Borders::ALL));
    frame.render_widget(list, area);
}

fn render_results(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let title = if app.search_query.is_empty() {
        "Memories".to_string()
    } else {
        format!("Results: {}", app.search_query)
    };
    let items: Vec<ListItem<'_>> = if app.search_query.is_empty() {
        app.memories
            .iter()
            .enumerate()
            .take(12)
            .map(|(index, memory)| {
                memory_item(
                    index,
                    app.selected_result,
                    &memory.ring,
                    &memory.summary,
                    None,
                )
            })
            .collect()
    } else if app.results.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "No matching memory.",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        app.results
            .iter()
            .enumerate()
            .map(|(index, result)| {
                memory_item(
                    index,
                    app.selected_result,
                    &result.memory.ring,
                    &result.memory.summary,
                    Some(result.score),
                )
            })
            .collect()
    };
    let list = List::new(items).block(Block::default().title(title).borders(Borders::ALL));
    frame.render_widget(list, area);
}

fn memory_item<'a>(
    index: usize,
    selected: usize,
    ring: &str,
    summary: &str,
    score: Option<f64>,
) -> ListItem<'a> {
    let marker = if index == selected { ">" } else { " " };
    let score = score
        .map(|score| format!(" score={score:.3}"))
        .unwrap_or_default();
    ListItem::new(Line::from(vec![
        Span::raw(marker.to_string()),
        Span::styled(format!(" [{ring}] "), Style::default().fg(Color::LightCyan)),
        Span::raw(truncate(summary, 80)),
        Span::styled(score, Style::default().fg(Color::DarkGray)),
    ]))
}

fn render_detail(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = Vec::new();
    if let Some(memory) = app.selected_memory() {
        let details = if memory.sensitivity == "normal" || app.include_sensitive {
            truncate(&memory.details, 220)
        } else {
            "[sensitive details hidden]".to_string()
        };
        lines.push(Line::from(vec![
            Span::styled("id ", Style::default().fg(Color::DarkGray)),
            Span::raw(memory.id.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("ring ", Style::default().fg(Color::DarkGray)),
            Span::raw(memory.ring.clone()),
            Span::styled(" type ", Style::default().fg(Color::DarkGray)),
            Span::raw(memory.event_type.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("confidence ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{:.2}", memory.confidence)),
            Span::styled(" salience ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{:.2}", memory.salience)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("source ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(
                "{} {}",
                memory.source.source_type, memory.source.ref_
            )),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(truncate(&memory.summary, 160)));
        if !details.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(details));
        }
    } else {
        lines.push(Line::from("No matching memory yet."));
        lines.push(Line::from("Use /remember <summary> or /search <query>."));
    }

    if app.mode == AppMode::Command {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("/", Style::default().fg(Color::LightYellow)),
            Span::raw(app.command_buffer.clone()),
        ]));
    } else if app.mode == AppMode::Search {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("search ", Style::default().fg(Color::LightYellow)),
            Span::raw(app.search_query.clone()),
        ]));
    }

    if !app.live_events.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "live",
            Style::default()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD),
        )));
        for event in app.live_events.iter().rev().take(3) {
            lines.push(Line::from(format!(
                "{} {}",
                event.ring.as_deref().unwrap_or("-"),
                event.safe_label()
            )));
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title("Detail / Actions")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let text = format!(
        "q quit | / command | s search | r rings | i sensitive:{} | u superseded:{} | {}",
        app.include_sensitive,
        app.include_superseded,
        command_help()
    );
    let footer = Paragraph::new(text)
        .block(Block::default().title("Actions").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(footer, area);
}

fn render_confirmation(frame: &mut Frame<'_>, area: Rect, prompt: &str) {
    frame.render_widget(ratatui::widgets::Clear, area);
    let paragraph = Paragraph::new(vec![
        Line::from(Span::styled(
            "Confirm Tree Ring Memory action",
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(prompt.to_string()),
    ])
    .block(Block::default().borders(Borders::ALL))
    .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_compact(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = vec![Line::from(Span::styled(
        "TREE RING MEMORY",
        Style::default()
            .fg(Color::LightYellow)
            .add_modifier(Modifier::BOLD),
    ))];
    lines.extend(ambient_tree_lines(&app.dashboard, app.tick));
    lines.push(Line::from(format!(
        "total {} | q quit | / command",
        app.dashboard.total
    )));
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    value
        .chars()
        .take(max.saturating_sub(3))
        .collect::<String>()
        + "..."
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tempfile::tempdir;

    use super::*;
    use crate::tui::app::App;

    #[test]
    fn render_buffer_contains_ambient_rings_and_actions() {
        let dir = tempdir().unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        app.execute_slash_command("/remember Use Rust TUI").unwrap();
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();

        assert!(output.contains("TREE RING MEMORY"));
        assert!(output.contains("Ambient Rings"));
        assert!(output.contains("Actions"));
        assert!(output.contains("cambium"));
    }
}
