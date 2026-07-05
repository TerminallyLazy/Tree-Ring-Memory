use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use super::app::{App, AppMode};
use super::input::command_help;
use super::rings::{ambient_corner_lines, ambient_tree_lines, exploded_ring_lines, ring_style};
use super::theme;

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    if area.width < 72 || area.height < 22 {
        render_compact(frame, area, app);
        return;
    }
    if area.width < 104 {
        render_narrow(frame, area, app);
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

fn render_narrow(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(11),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, vertical[0], app);
    if vertical[1].width >= 66 {
        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(30), Constraint::Length(32)])
            .split(vertical[1]);
        if app.mode == AppMode::Exploded {
            render_exploded(frame, top[0], app);
        } else {
            render_ring_activity(frame, top[0], app);
        }
        render_ambient_corner(frame, top[1], app);
    } else if app.mode == AppMode::Exploded {
        render_exploded(frame, vertical[1], app);
    } else {
        render_ring_activity(frame, vertical[1], app);
    }
    let lower = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(vertical[2]);
    render_ring_hud(frame, lower[0], app);
    render_results(frame, lower[1], app);
    render_footer(frame, vertical[3], app);

    if let Some(pending) = &app.pending_action {
        render_confirmation(
            frame,
            centered_rect(78, 30, area),
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
        AppMode::Integrations => "integrations",
    };
    let header = Paragraph::new(Line::from(vec![
        Span::styled("TREE RING MEMORY", theme::brand()),
        Span::styled(format!("  mode:{mode}"), theme::accent()),
        Span::styled(format!("  total:{}", app.dashboard.total), theme::title()),
        Span::styled(
            format!("  private:{}", app.dashboard.sensitive_total),
            if app.dashboard.sensitive_total > 0 {
                theme::warning()
            } else {
                theme::dim()
            },
        ),
        Span::styled("  status: ", theme::dim()),
        Span::raw(app.status.clone()),
    ]))
    .block(theme::plain_panel());
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
        .constraints([
            Constraint::Length(11),
            Constraint::Percentage(45),
            Constraint::Percentage(55),
        ])
        .split(columns[1]);

    if app.mode == AppMode::Exploded {
        render_exploded(frame, left[0], app);
    } else {
        render_ring_activity(frame, left[0], app);
    }
    render_ring_hud(frame, left[1], app);
    render_ambient_corner(frame, right[0], app);
    render_results(frame, right[1], app);
    render_detail(frame, right[2], app);
}

fn render_ambient_corner(frame: &mut Frame<'_>, area: Rect, app: &App) {
    if area.width < 28 || area.height < 10 {
        return;
    }
    frame.render_widget(Clear, area);
    let paragraph = Paragraph::new(ambient_corner_lines(&app.dashboard, app.tick))
        .block(theme::panel("Ambient Rings"))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_ring_activity(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let paragraph = Paragraph::new(exploded_ring_lines(&app.dashboard, app.selected_ring))
        .block(theme::panel("Ring Activity"))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_exploded(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let paragraph = Paragraph::new(exploded_ring_lines(&app.dashboard, app.selected_ring))
        .block(theme::panel("/rings"))
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
            let selector_style = if index == app.selected_ring {
                theme::secondary_accent().add_modifier(Modifier::BOLD)
            } else {
                theme::dim()
            };
            let line = Line::from(vec![
                Span::styled(selected, selector_style),
                Span::styled(format!(" {:<10}", stats.ring), ring_style(stats)),
                Span::styled(
                    format!(" {:>4}", stats.total),
                    if index == app.selected_ring {
                        theme::selected()
                    } else {
                        theme::title()
                    },
                ),
                Span::styled(
                    format!(
                        " avg {:.2}/{:.2} private {}",
                        stats.average_confidence, stats.average_salience, stats.sensitive_count
                    ),
                    theme::dim(),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();
    let list = List::new(items).block(theme::panel("Rings"));
    frame.render_widget(list, area);
}

fn render_results(frame: &mut Frame<'_>, area: Rect, app: &App) {
    if app.mode == AppMode::Integrations {
        render_integrations(frame, area, app);
        return;
    }
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
            theme::dim(),
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
    let list = List::new(items).block(theme::panel(title));
    frame.render_widget(list, area);
}

fn render_integrations(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let items = app
        .integration_report
        .as_ref()
        .map(|report| {
            report
                .integrations
                .iter()
                .map(|integration| {
                    let detected =
                        integration.status == crate::integrations::IntegrationStatus::Detected;
                    let marker = if detected { "*" } else { " " };
                    let style = if detected {
                        theme::selected()
                    } else {
                        theme::dim()
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(marker, theme::secondary_accent()),
                        Span::styled(format!(" {:<18}", integration.name), style),
                        Span::styled(
                            format!(" {:?} {:.2}", integration.status, integration.confidence),
                            theme::dim(),
                        ),
                    ]))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            vec![ListItem::new(Line::from(Span::styled(
                "Run /integrations to scan.",
                theme::dim(),
            )))]
        });
    let title = app
        .integration_report
        .as_ref()
        .map(|report| format!("Agent Frameworks: {} detected", report.detected_count))
        .unwrap_or_else(|| "Agent Frameworks".to_string());
    frame.render_widget(List::new(items).block(theme::panel(title)), area);
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
    let selected_row = index == selected;
    let memory_style = if selected_row {
        theme::selected()
    } else {
        Style::default()
    };
    ListItem::new(Line::from(vec![
        Span::styled(
            marker.to_string(),
            if selected_row {
                theme::secondary_accent().add_modifier(Modifier::BOLD)
            } else {
                theme::dim()
            },
        ),
        Span::styled(
            format!(" [{ring}] "),
            Style::default()
                .fg(theme::ring_color(ring, 0.0))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(truncate(summary, 80), memory_style),
        Span::styled(score, theme::dim()),
    ]))
}

fn render_detail(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = Vec::new();
    if app.mode == AppMode::Integrations {
        if let Some(report) = &app.integration_report {
            lines.push(Line::from(vec![
                Span::styled("root ", theme::dim()),
                Span::raw(report.root.display().to_string()),
            ]));
            lines.push(Line::from(""));
            for integration in report.integrations.iter().take(6) {
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", integration.name), theme::brand()),
                    Span::styled(format!("{:?}", integration.status), theme::dim()),
                ]));
                lines.push(Line::from(truncate(integration.next_step, 140)));
                if !integration.markers.is_empty() {
                    lines.push(Line::from(Span::styled(
                        truncate(&format!("markers: {}", integration.markers.join(", ")), 140),
                        theme::dim(),
                    )));
                }
                lines.push(Line::from(""));
            }
        } else {
            lines.push(Line::from("Run /integrations to scan local agent markers."));
        }
    } else if let Some(memory) = app.selected_memory() {
        let details = if memory.sensitivity == "normal" || app.include_sensitive {
            truncate(&memory.details, 220)
        } else {
            "[sensitive details hidden]".to_string()
        };
        lines.push(Line::from(vec![
            Span::styled("id ", theme::dim()),
            Span::raw(memory.id.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("ring ", theme::dim()),
            Span::styled(
                memory.ring.clone(),
                Style::default()
                    .fg(theme::ring_color(&memory.ring, 0.0))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" type ", theme::dim()),
            Span::raw(memory.event_type.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("confidence ", theme::dim()),
            Span::styled(format!("{:.2}", memory.confidence), theme::accent()),
            Span::styled(" salience ", theme::dim()),
            Span::styled(format!("{:.2}", memory.salience), theme::secondary_accent()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("source ", theme::dim()),
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
            Span::styled("/", theme::brand()),
            Span::raw(app.command_buffer.clone()),
        ]));
    } else if app.mode == AppMode::Search {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("search ", theme::brand()),
            Span::raw(app.search_query.clone()),
        ]));
    }

    if !app.live_events.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("live", theme::live())));
        for event in app.live_events.iter().rev().take(3) {
            let ring = event.ring.as_deref().unwrap_or("-");
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{ring:<10} "),
                    Style::default()
                        .fg(theme::ring_color(ring, 0.0))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(event.safe_label()),
            ]));
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(theme::panel("Detail / Actions"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let text = if area.width < 104 {
        format!(
            "q quit | / cmd | s search | r rings | i sens:{} | u super:{}",
            app.include_sensitive, app.include_superseded
        )
    } else {
        format!(
            "q quit | / command | s search | r rings | i sensitive:{} | u superseded:{} | {}",
            app.include_sensitive,
            app.include_superseded,
            command_help()
        )
    };
    let footer = Paragraph::new(text)
        .style(theme::accent())
        .block(theme::panel("Actions"))
        .wrap(Wrap { trim: true });
    frame.render_widget(footer, area);
}

fn render_confirmation(frame: &mut Frame<'_>, area: Rect, prompt: &str) {
    frame.render_widget(Clear, area);
    let paragraph = Paragraph::new(vec![
        Line::from(Span::styled(
            "Confirm Tree Ring Memory action",
            theme::warning(),
        )),
        Line::from(""),
        Line::from(prompt.to_string()),
    ])
    .block(theme::plain_panel().border_style(theme::warning()))
    .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_compact(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = vec![Line::from(Span::styled("TREE RING MEMORY", theme::brand()))];
    lines.extend(ambient_tree_lines(&app.dashboard, app.tick));
    lines.push(Line::from(format!(
        "total {} | q quit | / command",
        app.dashboard.total
    )));
    let paragraph = Paragraph::new(lines)
        .block(theme::plain_panel())
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
        assert!(output.contains("Ring Activity"));
        assert!(output.contains("Actions"));
        assert!(output.contains("cambium"));
    }

    #[test]
    fn render_narrow_buffer_keeps_ring_and_footer_visible() {
        let dir = tempdir().unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        app.execute_slash_command("/remember Use Rust TUI").unwrap();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();

        assert!(output.contains("Ambient Rings"));
        assert!(output.contains("live"));
        assert!(output.contains("heartwood"));
        assert!(output.contains("u super:false"));
    }
}
