use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use super::actions::{ActionKind, PendingAction};
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
            confirmation_rect(area),
            pending,
            app.include_sensitive,
        );
    }
}

fn render_narrow(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(12),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, vertical[0], app);
    if vertical[1].width >= 66 {
        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(30), Constraint::Length(36)])
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
            confirmation_rect(area),
            pending,
            app.include_sensitive,
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
        AppMode::Evidence => "evidence",
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
    let right = if app.mode == AppMode::Evidence {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10),
                Constraint::Length(6),
                Constraint::Min(12),
            ])
            .split(columns[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(14),
                Constraint::Percentage(45),
                Constraint::Percentage(55),
            ])
            .split(columns[1])
    };

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
    if area.width < 34 || area.height < 11 {
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
    if app.mode == AppMode::Evidence {
        render_evidence_list(frame, area, app);
        return;
    }
    if app.mode == AppMode::Integrations {
        render_integrations(frame, area, app);
        return;
    }
    let title = if app.search_query.trim().is_empty() {
        "Memories".to_string()
    } else {
        format!("Results: {}", app.search_query)
    };
    let items: Vec<ListItem<'_>> = if app.search_query.trim().is_empty() && app.memories.is_empty()
    {
        let message = if app.dashboard.total == 0 {
            "No stored memories yet. /sync previews DOX; /remember saves a lesson."
        } else {
            "Memories are hidden by visibility filters. i: sensitive, u: superseded."
        };
        vec![ListItem::new(Line::from(Span::styled(
            message,
            theme::dim(),
        )))]
    } else if app.search_query.trim().is_empty() {
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
                    let detected = integration.status
                        == tree_ring_memory_cli::activation::adapters::IntegrationStatus::Detected;
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
                            format!(" {:?} {:?}", integration.status, integration.state),
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

fn render_evidence_list(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let items = if let Some(snapshot) = &app.evidence_snapshot {
        let mut rows = Vec::new();
        let status = snapshot.status.as_str();
        rows.push(ListItem::new(Line::from(vec![
            Span::styled("* ", theme::secondary_accent()),
            Span::styled("Certification", theme::selected()),
            Span::styled(format!(" {status}"), theme::dim()),
        ])));

        let harness_status = snapshot
            .index
            .as_ref()
            .map(|index| {
                if index.harness.is_empty() {
                    if index.missing.iter().any(|item| item == "harness") {
                        " missing"
                    } else {
                        " none"
                    }
                } else {
                    " loaded"
                }
            })
            .unwrap_or(" missing");
        let recall_status = if let Some(recall_quality) = snapshot.recall_quality.as_ref() {
            format!(" {}", recall_quality.status.as_str())
        } else {
            snapshot
                .index
                .as_ref()
                .map(|index| {
                    if let Some(record) = &index.recall_quality {
                        format!(" {}", record.status.as_str())
                    } else if index.missing.iter().any(|item| item == "recall_quality") {
                        " missing".to_string()
                    } else {
                        " none".to_string()
                    }
                })
                .unwrap_or(" missing".to_string())
        };

        if let Some(index) = snapshot.index.as_ref() {
            if index.harness.is_empty() {
                rows.push(ListItem::new(Line::from(vec![
                    Span::styled("  ", theme::dim()),
                    Span::styled("Harness probes", theme::dim()),
                    Span::styled(harness_status, theme::dim()),
                ])));
            } else {
                for record in index.harness.values().take(6) {
                    rows.push(ListItem::new(Line::from(vec![
                        Span::styled("  ", theme::dim()),
                        Span::styled(format!("{:<18}", record.label), theme::selected()),
                        Span::styled(format!(" {}", record.status.as_str()), theme::dim()),
                    ])));
                }
            }
        } else {
            rows.push(ListItem::new(Line::from(vec![
                Span::styled("  ", theme::dim()),
                Span::styled("Harness probes", theme::dim()),
                Span::styled(harness_status, theme::dim()),
            ])));
        }
        rows.push(ListItem::new(Line::from(vec![
            Span::styled("  ", theme::dim()),
            Span::styled("Recall quality", theme::dim()),
            Span::styled(recall_status, theme::dim()),
        ])));
        if let Some(certification) = &snapshot.certification {
            if let Some(status) = &certification.agent_zero_status {
                rows.push(ListItem::new(Line::from(vec![
                    Span::styled("  ", theme::dim()),
                    Span::styled("Agent Zero", theme::dim()),
                    Span::styled(format!(" {status}"), theme::dim()),
                ])));
            }
        }
        rows
    } else {
        vec![ListItem::new(Line::from(Span::styled(
            "Run /evidence to load proof.",
            theme::dim(),
        )))]
    };
    frame.render_widget(List::new(items).block(theme::panel("Evidence")), area);
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
    if app.mode == AppMode::Evidence {
        render_evidence_detail(frame, area, app);
        return;
    }
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
                lines.push(Line::from(truncate(&integration.next_step, 140)));
                if !integration.markers.is_empty() {
                    lines.push(Line::from(Span::styled(
                        truncate(
                            &format!(
                                "markers: {}",
                                tree_ring_memory_cli::activation::adapters::format_markers(
                                    &integration.markers,
                                )
                            ),
                            140,
                        ),
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
        if app.dashboard.total == 0 {
            lines.push(Line::from("No stored memories yet."));
            lines.push(Line::from("A recall receipt does not create memory."));
            lines.push(Line::from(
                "Use /sync to review DOX summaries or /remember <lesson>.",
            ));
        } else {
            lines.push(Line::from("No visible matching memory."));
            lines.push(Line::from("Clear search or review i/u visibility filters."));
        }
        lines.push(Line::from(format!("Store: {}", app.store_path().display())));
    }

    if app.status.starts_with("action failed:")
        || app.status.starts_with("command failed:")
        || app.status.starts_with("DOX sync:")
    {
        lines.insert(0, Line::from(app.status.clone()));
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

fn render_evidence_detail(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = Vec::new();
    if let Some(snapshot) = &app.evidence_snapshot {
        lines.push(Line::from(vec![
            Span::styled("status ", theme::dim()),
            Span::styled(snapshot.status.as_str(), theme::accent()),
        ]));
        if let Some(certification) = &snapshot.certification {
            lines.push(Line::from(vec![
                Span::styled("Local certification ", theme::brand()),
                Span::styled(
                    format!(
                        "{} generated {}",
                        certification.status.as_str(),
                        certification.generated_at
                    ),
                    theme::dim(),
                ),
            ]));
            let mut install_parts = Vec::new();
            if let Some(bytes) = certification.release_binary_bytes {
                install_parts.push(format!("release {bytes} bytes"));
            }
            if let Some(project_kb) = certification.project_install_kb {
                install_parts.push(format!("project {project_kb} KB"));
            }
            if let Some(global_kb) = certification.global_install_kb {
                install_parts.push(format!("global {global_kb} KB"));
            }
            if !install_parts.is_empty() {
                lines.push(Line::from(format!("install {}", install_parts.join(" | "))));
            }
            if let Some(avg) = certification.recall_avg_ms_10000 {
                let max = certification.recall_max_ms_10000.unwrap_or(avg);
                let mut line = format!("10k recall {avg:.3} ms max {max:.3} ms");
                if let Some(rate) = certification.cli_import_events_per_second {
                    line.push_str(&format!(" | import {rate}/s"));
                }
                lines.push(Line::from(line));
            } else if let Some(rate) = certification.cli_import_events_per_second {
                lines.push(Line::from(format!("CLI import {rate}/s")));
            }
            if let Some(avg) = certification.recall_avg_ms_30000 {
                let max = certification.recall_max_ms_30000.unwrap_or(avg);
                lines.push(Line::from(format!(
                    "30k recall {avg:.3} ms max {max:.3} ms"
                )));
            }
            if let Some(status) = &certification.agent_zero_status {
                lines.push(Line::from(format!(
                    "Agent Zero {} {}",
                    status,
                    certification.agent_zero_note.as_deref().unwrap_or("")
                )));
            }
            lines.push(Line::from(vec![
                Span::styled("root ", theme::dim()),
                Span::raw(truncate(&snapshot.root.display().to_string(), 56)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("index ", theme::dim()),
                Span::raw(truncate(&snapshot.index_path.display().to_string(), 55)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("metrics ", theme::dim()),
                Span::raw(truncate(
                    &certification.metrics_path.display().to_string(),
                    53,
                )),
            ]));
        } else {
            lines.push(Line::from(snapshot.message.clone()));
            lines.push(Line::from("Run: sh scripts/certify-tree-ring.sh"));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("root ", theme::dim()),
                Span::raw(truncate(&snapshot.root.display().to_string(), 56)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("index ", theme::dim()),
                Span::raw(truncate(&snapshot.index_path.display().to_string(), 55)),
            ]));
        }
        if let Some(index) = snapshot.index.as_ref() {
            if !index.harness.is_empty() {
                lines.push(Line::from(Span::styled("Harness matrix", theme::brand())));
                for record in index.harness.values().take(6) {
                    lines.push(Line::from(vec![
                        Span::styled(truncate(&record.label, 18), theme::selected()),
                        Span::raw(" "),
                        Span::styled(record.status.as_str(), theme::dim()),
                        Span::raw(" "),
                        Span::raw(truncate(&record.path.display().to_string(), 36)),
                    ]));
                }
            }
        }
        if let Some(recall_quality) = &snapshot.recall_quality {
            lines.push(Line::from(vec![
                Span::styled("Recall quality ", theme::brand()),
                Span::styled(recall_quality.status.as_str(), theme::dim()),
            ]));
            lines.push(Line::from(format!(
                "{} queries {} pass {} fail {} review {}",
                recall_quality.query_set_id,
                recall_quality.query_count,
                recall_quality.pass_count,
                recall_quality.fail_count,
                recall_quality.needs_review_count
            )));
            if let Some(avg) = recall_quality.avg_latency_ms {
                let max = recall_quality.max_latency_ms.unwrap_or(avg);
                lines.push(Line::from(format!("avg {avg:.3} ms max {max:.3} ms")));
            }
            lines.push(Line::from(vec![
                Span::styled("record ", theme::dim()),
                Span::raw(truncate(
                    &recall_quality.record_path.display().to_string(),
                    52,
                )),
            ]));
            for query in recall_quality.queries.iter().take(4) {
                let returned = if query.returned_ids.is_empty() {
                    "-".to_string()
                } else {
                    query.returned_ids.join(",")
                };
                let rank = query
                    .expected_rank
                    .map(|rank| rank.to_string())
                    .unwrap_or_else(|| "-".to_string());
                lines.push(Line::from(format!(
                    "{} [{}] rank {} {:.3}ms",
                    truncate(&query.query_id, 28),
                    query.status,
                    rank,
                    query.latency_ms.unwrap_or(0.0)
                )));
                lines.push(Line::from(Span::styled(
                    truncate(&format!("returned {returned}"), 70),
                    theme::dim(),
                )));
            }
        }
        lines.push(Line::from("Actions: /evidence refresh | /integrations"));
    } else {
        lines.push(Line::from("Run /evidence to load certification proof."));
    }
    let paragraph = Paragraph::new(lines)
        .block(theme::panel("Evidence Detail"))
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

fn render_confirmation(
    frame: &mut Frame<'_>,
    area: Rect,
    pending: &PendingAction,
    include_sensitive: bool,
) {
    frame.render_widget(Clear, area);
    let block = theme::panel("Confirm Tree Ring Memory action").border_style(theme::warning());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if let ActionKind::SyncDox {
        preview,
        selected_candidate,
        preview_scroll,
    } = &pending.kind
    {
        let regions = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(inner);
        if let Some(event) = preview.events.get(*selected_candidate) {
            let detail = if event.sensitivity == "normal" || include_sensitive {
                format!(
                    "Candidate {}/{} [{}] source: {}\n{}",
                    selected_candidate + 1,
                    preview.events.len(),
                    event.ring,
                    event.source.ref_,
                    event.summary,
                )
            } else {
                format!("Candidate {}/{} [{}]\nSensitive candidate hidden. Press i to review. Confirm all includes this candidate.", selected_candidate + 1, preview.events.len(), event.ring)
            };
            let lines = wrap_preview_text(
                &format!("{}\n\n{detail}", pending.summary),
                regions[0].width,
            );
            let max_scroll = lines
                .len()
                .saturating_sub(usize::from(regions[0].height))
                .min(usize::from(u16::MAX)) as u16;
            let offset = preview_scroll.get().min(max_scroll);
            // This is viewport state only. Clamp after resize/content changes
            // so a single reverse-scroll key always moves the visible text.
            preview_scroll.set(offset);
            frame.render_widget(Paragraph::new(lines).scroll((offset, 0)), regions[0]);
        }
        frame.render_widget(
            Paragraph::new(
                "j/k item; Left/Right/Pg scroll; i sensitive\ny confirm all; n/Esc cancel; Home/End item",
            )
            .style(theme::warning()),
            regions[1],
        );
    } else {
        let regions = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(inner);
        frame.render_widget(
            Paragraph::new(pending.summary.clone()).wrap(Wrap { trim: true }),
            regions[0],
        );
        frame.render_widget(
            Paragraph::new("press y to confirm, n/Esc to cancel")
                .wrap(Wrap { trim: true })
                .style(theme::warning()),
            regions[1],
        );
    }
}

fn render_compact(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = vec![Line::from(Span::styled("TREE RING MEMORY", theme::brand()))];
    lines.push(Line::from(format!(
        "total {} | q quit | / command",
        app.dashboard.total
    )));
    if app.mode == AppMode::Command {
        lines.push(Line::from(format!("/{}", app.command_buffer)));
    }
    lines.push(Line::from(app.status.clone()));
    if app.mode != AppMode::Command && app.dashboard.total == 0 {
        lines.push(Line::from(
            "No stored memories. /sync previews DOX; /remember saves a lesson.",
        ));
    }
    lines.extend(ambient_tree_lines(&app.dashboard, app.tick));
    let paragraph = Paragraph::new(lines)
        .block(theme::plain_panel())
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
    if let Some(pending) = &app.pending_action {
        render_confirmation(frame, area, pending, app.include_sensitive);
    }
}

// Wrap once into explicit display rows so preview scrolling has an exact
// bound, including long source paths without whitespace.
fn wrap_preview_text(text: &str, width: u16) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let mut output = Vec::new();
    for line in text.lines() {
        let mut row = String::new();
        let mut columns = 0;
        for character in line.chars() {
            let character_width = Span::raw(character.to_string()).width();
            if columns + character_width > width && !row.is_empty() {
                output.push(Line::from(std::mem::take(&mut row)));
                columns = 0;
            }
            row.push(character);
            columns += character_width;
        }
        output.push(Line::from(row));
    }
    output
}

fn confirmation_rect(area: Rect) -> Rect {
    let width = area.width.saturating_sub(4).min(110);
    let height = area.height.saturating_sub(2);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
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
    fn long_dox_candidate_can_scroll_to_its_end_with_fixed_confirmation_keys() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("AGENTS.md"),
            "# Rules\n\nReview source contracts.\n",
        )
        .unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        app.execute_slash_command("/sync").unwrap();
        if let ActionKind::SyncDox { preview, .. } = &mut app.pending_action.as_mut().unwrap().kind
        {
            preview.events[0].source.ref_ =
                format!("{}AGENTS.md#rules", "long-source-directory/".repeat(8));
            preview.events[0].summary = format!(
                "{}\nFINAL-REVIEW-SENTINEL",
                "Review bounded source guidance. ".repeat(40)
            );
        }
        let mut terminal = Terminal::new(TestBackend::new(60, 18)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert!(!terminal
            .backend()
            .to_string()
            .contains("FINAL-REVIEW-SENTINEL"));
        let mut reached_end = false;
        for _ in 0..10 {
            app.handle_key(ratatui::crossterm::event::KeyEvent::new(
                ratatui::crossterm::event::KeyCode::PageDown,
                ratatui::crossterm::event::KeyModifiers::NONE,
            ))
            .unwrap();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            let output = terminal.backend().to_string();
            assert!(output.contains("y confirm all; n/Esc cancel"));
            if output.contains("FINAL-REVIEW-SENTINEL") {
                reached_end = true;
                break;
            }
        }
        assert!(
            reached_end,
            "full candidate must be reviewable before confirmation"
        );
        assert!(app.store.list_all(true).unwrap().is_empty());
    }

    #[test]
    fn compact_layout_prioritizes_command_and_actionable_errors_over_art() {
        let dir = tempdir().unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        app.status = "action failed: authorization denied: coordinator capability required by coordinated store policy. Relaunch the TUI with TREE_RING_COORDINATOR_TOKEN supplied by your existing secure environment.".to_string();
        app.mode = AppMode::Command;
        app.command_buffer = "sync".to_string();
        let mut terminal = Terminal::new(TestBackend::new(60, 14)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();
        assert!(output.contains("/sync"));
        assert!(output.contains("Relaunch"), "{output}");
        assert!(output.contains("TREE_RING_COORDINATOR_TOKEN"));
        assert!(output.contains("secure"), "{output}");
        assert!(output.contains("environment"), "{output}");
    }

    #[test]
    fn dox_preview_shows_sources_candidates_and_confirmation_at_all_layout_sizes() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("AGENTS.md"),
            "# Project rules\n\nRead source contracts before editing.\n",
        )
        .unwrap();
        std::fs::create_dir(dir.path().join(".spynel")).unwrap();
        std::fs::write(
            dir.path().join(".spynel/AGENTS.md"),
            "# Local rules\n\nVerify local configuration.\n",
        )
        .unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        app.execute_slash_command("/sync").unwrap();
        for (width, height) in [(120, 36), (80, 24), (64, 20)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            let output = terminal.backend().to_string();
            assert!(
                output.contains("2 DOX summaries"),
                "{width}x{height}: {output}"
            );
            assert!(output.contains("Source:"));
            assert!(output.contains("Store:"));
            assert!(output.contains("Project:"));
            assert!(output.contains("Candidate 1/2"));
            assert!(output.contains(".spynel/AGENTS.md"));
            assert!(output.contains("Verify"), "{width}x{height}: {output}");
            assert!(
                output.contains("DOX verification"),
                "{width}x{height}: {output}"
            );
            assert!(output.contains("y confirm all; n/Esc cancel"));
        }
        app.handle_key(ratatui::crossterm::event::KeyEvent::new(
            ratatui::crossterm::event::KeyCode::End,
            ratatui::crossterm::event::KeyModifiers::NONE,
        ))
        .unwrap();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();
        assert!(output.contains("Candidate 2/2"));
        assert!(output.contains("Read source contracts"));
        assert!(app.store.list_all(true).unwrap().is_empty());
    }

    #[test]
    fn dox_preview_hides_sensitive_candidate_and_source_until_explicit_opt_in() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("AGENTS.md"),
            "# Rules\n\nReview source contracts.\n",
        )
        .unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        app.execute_slash_command("/sync").unwrap();
        if let ActionKind::SyncDox { preview, .. } = &mut app.pending_action.as_mut().unwrap().kind
        {
            preview.events[0].sensitivity = "sensitive".to_string();
            preview.events[0].summary = "SENSITIVE-CANDIDATE-SENTINEL".to_string();
            preview.events[0].source.ref_ = "SENSITIVE-SOURCE-SENTINEL".to_string();
        }
        for (width, height) in [(120, 36), (80, 24), (64, 20)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            let output = terminal.backend().to_string();
            assert!(output.contains("Sensitive candidate hidden"));
            assert!(!output.contains("SENSITIVE-CANDIDATE-SENTINEL"));
            assert!(!output.contains("SENSITIVE-SOURCE-SENTINEL"));
        }
        app.handle_key(ratatui::crossterm::event::KeyEvent::new(
            ratatui::crossterm::event::KeyCode::Char('i'),
            ratatui::crossterm::event::KeyModifiers::NONE,
        ))
        .unwrap();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();
        assert!(output.contains("SENSITIVE-CANDIDATE-SENTINEL"));
        assert!(output.contains("SENSITIVE-SOURCE-SENTINEL"));
        assert!(app.store.list_all(true).unwrap().is_empty());
    }

    #[test]
    fn hiding_sensitive_preview_clears_cached_memories_before_cancel_redraw() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("AGENTS.md"),
            "# Rules\n\nReview source contracts.\n",
        )
        .unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        let mut private =
            tree_ring_memory_core::MemoryEvent::new("PRIVATE-CACHE-SENTINEL", "lesson").unwrap();
        private.sensitivity = "private".to_string();
        private.source.ref_ = "PRIVATE-SOURCE-CACHE-SENTINEL".to_string();
        app.store.put(&private).unwrap();
        app.include_sensitive = true;
        app.refresh_store().unwrap();
        assert_eq!(app.memories.len(), 1);
        app.execute_slash_command("/sync").unwrap();

        for code in [
            ratatui::crossterm::event::KeyCode::Char('i'),
            ratatui::crossterm::event::KeyCode::Esc,
        ] {
            app.handle_key(ratatui::crossterm::event::KeyEvent::new(
                code,
                ratatui::crossterm::event::KeyModifiers::NONE,
            ))
            .unwrap();
        }

        assert!(!app.include_sensitive);
        assert!(app.pending_action.is_none());
        // Render before any event-loop tick can refresh the cached view.
        let mut terminal = Terminal::new(TestBackend::new(120, 36)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();
        assert!(!output.contains("PRIVATE-CACHE-SENTINEL"));
        assert!(!output.contains("PRIVATE-SOURCE-CACHE-SENTINEL"));
        assert!(app.memories.is_empty());
        assert_eq!(app.store.list_all(true).unwrap().len(), 1);
    }

    #[test]
    fn empty_store_explains_population_and_preserves_sensitive_filter_distinction() {
        let dir = tempdir().unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(120, 36)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();
        assert!(output.contains("No stored memories yet"));
        assert!(output.contains("recall receipt does not create memory"));
        assert!(output.contains("/sync"));
        assert!(output.contains("Store:"));

        let mut private =
            tree_ring_memory_core::MemoryEvent::new("PRIVATE-CONTENT-SENTINEL", "lesson").unwrap();
        private.sensitivity = "private".to_string();
        app.store.put(&private).unwrap();
        app.refresh_store().unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();
        assert!(output.contains("visibility filters"));
        assert!(!output.contains("PRIVATE-CONTENT-SENTINEL"));
        assert!(!output.contains("No stored memories yet"));
    }

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

    #[test]
    fn render_evidence_mode_shows_empty_state_and_refresh_command() {
        let dir = tempdir().unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        app.execute_slash_command("/evidence").unwrap();
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();

        assert!(output.contains("Evidence"));
        assert!(output.contains("missing"));
        assert!(output.contains("certify-tree-ring"));
    }

    #[test]
    fn render_evidence_mode_shows_certification_metrics() {
        let dir = tempdir().unwrap();
        let evidence_dir = dir.path().join("target/tree-ring-certification");
        std::fs::create_dir_all(&evidence_dir).unwrap();
        std::fs::write(evidence_dir.join("summary.md"), "# Summary\n").unwrap();
        std::fs::write(
            evidence_dir.join("metrics.json"),
            r#"{
              "ok": true,
              "created_at": "2026-07-09T04:22:38Z",
              "release_binary_bytes": 6137088,
              "project_install_kb": 6064,
              "global_install_kb": 6020,
              "cli_import": {"events_per_second": 2000},
              "performance": {
                "records_10000": {"recall_avg_ms": 3.729, "recall_max_ms": 6.539},
                "records_30000": {"recall_avg_ms": 7.978, "recall_max_ms": 14.444}
              },
              "agent_zero": {"status": "skipped", "note": "TREE_RING_AGENT_ZERO_ROOT not set"}
            }"#,
        )
        .unwrap();
        std::fs::write(
            evidence_dir.join("evidence-index.json"),
            r#"{
              "generated_at": "2026-07-09T04:22:38Z",
              "overall_status": "pass",
              "certification": {
                "category": "certification",
                "status": "pass",
                "label": "Local certification",
                "path": "metrics.json",
                "summary_path": "summary.md",
                "generated_at": "2026-07-09T04:22:38Z"
              },
              "harness": {},
              "recall_quality": null,
              "missing": ["harness", "recall_quality"],
              "stale": []
            }"#,
        )
        .unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        app.execute_slash_command("/evidence").unwrap();
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();

        assert!(output.contains("Local certification"));
        assert!(output.contains("6064 KB"));
        assert!(output.contains("3.729 ms"));
        assert!(output.contains("Agent Zero"));
    }

    #[test]
    fn render_evidence_mode_shows_harness_matrix_records() {
        let dir = tempdir().unwrap();
        let evidence_dir = dir.path().join("target/tree-ring-certification");
        std::fs::create_dir_all(evidence_dir.join("harness")).unwrap();
        std::fs::write(evidence_dir.join("summary.md"), "# Summary\n").unwrap();
        std::fs::write(
            evidence_dir.join("metrics.json"),
            r#"{"ok":true,"created_at":"2026-07-09T05:44:48Z"}"#,
        )
        .unwrap();
        std::fs::write(evidence_dir.join("harness/codex.json"), "{}").unwrap();
        std::fs::write(evidence_dir.join("harness/claude-code.json"), "{}").unwrap();
        std::fs::write(
            evidence_dir.join("evidence-index.json"),
            r#"{
          "generated_at": "2026-07-09T05:44:48Z",
          "overall_status": "fail",
          "certification": {
            "category": "certification",
            "status": "pass",
            "label": "Local certification",
            "path": "metrics.json",
            "summary_path": "summary.md",
            "generated_at": "2026-07-09T05:44:48Z"
          },
          "harness": {
            "codex": {
              "category": "harness",
              "status": "pass",
              "label": "Codex",
              "path": "harness/codex.json",
              "summary_path": null,
              "generated_at": "2026-07-09T05:44:48Z"
            },
            "claude-code": {
              "category": "harness",
              "status": "fail",
              "label": "Claude Code",
              "path": "harness/claude-code.json",
              "summary_path": null,
              "generated_at": "2026-07-09T05:44:48Z"
            }
          },
          "recall_quality": null,
          "missing": ["recall_quality"],
          "stale": []
        }"#,
        )
        .unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        app.execute_slash_command("/evidence").unwrap();
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();

        assert!(output.contains("status"));
        assert!(output.contains("Local certification"));
        assert!(output.contains("root"));
        assert!(output.contains("index"));
        assert!(output.contains("metrics"));
        assert!(output.contains("q quit | / command"));
        assert!(output.contains("Codex"));
        assert!(output.contains("pass"));
        assert!(output.contains("Claude Code"));
        assert!(output.contains("fail"));
        assert!(output.contains("codex.json"));
    }

    #[test]
    fn render_evidence_mode_shows_recall_quality_records() {
        let dir = tempdir().unwrap();
        let evidence_dir = dir.path().join("target/tree-ring-certification");
        std::fs::create_dir_all(evidence_dir.join("recall-quality")).unwrap();
        std::fs::write(
            evidence_dir.join("metrics.json"),
            r#"{"ok":true,"created_at":"2026-07-09T05:44:48Z"}"#,
        )
        .unwrap();
        std::fs::write(
            evidence_dir.join("recall-quality/default-fixture-v1.json"),
            r#"{
          "schema_version": 1,
          "generated_at": "2026-07-09T06:00:00Z",
          "query_set_id": "default-fixture-v1",
          "status": "needs_review",
          "summary": {
            "query_count": 4,
            "pass_count": 3,
            "fail_count": 0,
            "needs_review_count": 1,
            "avg_latency_ms": 0.5,
            "max_latency_ms": 1.0
          },
          "queries": [
            {
              "query_id": "scar-stale-cache",
              "query": "failure stale cache",
              "status": "pass",
              "expected_top_id": "rq_scar_stale_cache",
              "expected_rank": 1,
              "latency_ms": 0.25,
              "returned": [
                {"id":"rq_scar_stale_cache","rank":1,"ring":"scar","source_ref":"recall-quality/scar-stale-cache","score":1.2,"ranking":{"textual_match":1.0}}
              ],
              "notes": []
            }
            ,
            {
              "query_id": "scar-no-hit",
              "query": "cache miss",
              "status": "needs_review",
              "expected_top_id": null,
              "expected_rank": null,
              "latency_ms": 0.75,
              "returned": [],
              "notes": []
            }
          ]
        }"#,
        )
        .unwrap();
        std::fs::write(
            evidence_dir.join("evidence-index.json"),
            r#"{
          "generated_at": "2026-07-09T06:00:00Z",
          "overall_status": "pass",
          "certification": {
            "category": "certification",
            "status": "pass",
            "label": "Local certification",
            "path": "metrics.json",
            "summary_path": null,
            "generated_at": "2026-07-09T05:44:48Z"
          },
          "harness": {},
          "recall_quality": {
            "category": "recall_quality",
            "status": "pass",
            "label": "Recall quality",
            "path": "recall-quality/default-fixture-v1.json",
            "summary_path": null,
            "generated_at": "2026-07-09T06:00:00Z"
          },
          "missing": [],
          "stale": []
        }"#,
        )
        .unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        app.execute_slash_command("/evidence").unwrap();
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();

        assert!(output.contains("Recall quality needs_review"));
        assert!(!output.contains("Recall quality pass"));
        assert!(output.contains("default-fixture-v1"));
        assert!(output.contains("queries 4"));
        assert!(output.contains("avg 0.500 ms"));
        assert!(output.contains("scar-stale-cache"));
        assert!(output.contains("rank 1"));
        assert!(output.contains("scar-no-hit"));
        assert!(output.contains("rank -"));
        assert!(output.contains("rq_scar_stale_cache"));
    }

    #[test]
    fn render_evidence_mode_keeps_status_without_certification() {
        let dir = tempdir().unwrap();
        let evidence_dir = dir.path().join("target/tree-ring-certification");
        std::fs::create_dir_all(evidence_dir.join("harness")).unwrap();
        std::fs::write(evidence_dir.join("harness/codex.json"), "{}").unwrap();
        std::fs::write(
            evidence_dir.join("evidence-index.json"),
            r#"{
          "generated_at": "2026-07-09T05:44:48Z",
          "overall_status": "fail",
          "certification": null,
          "harness": {
            "codex": {
              "category": "harness",
              "status": "pass",
              "label": "Codex",
              "path": "harness/codex.json",
              "summary_path": null,
              "generated_at": "2026-07-09T05:44:48Z"
            }
          },
          "recall_quality": null,
          "missing": ["certification", "recall_quality"],
          "stale": []
        }"#,
        )
        .unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        app.execute_slash_command("/evidence").unwrap();
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();

        assert!(output.contains("status"));
        assert!(output.contains("fail"));
        assert!(output.contains("Harness matrix"));
        assert!(output.contains("Codex"));
        assert!(!output.contains("Local certification"));
    }

    #[test]
    fn render_evidence_mode_keeps_harness_fallback_when_index_missing() {
        let dir = tempdir().unwrap();
        let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
        app.execute_slash_command("/evidence").unwrap();
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let output = terminal.backend().to_string();

        assert!(output.contains("Harness probes"));
        assert!(output.contains("missing"));
    }
}
