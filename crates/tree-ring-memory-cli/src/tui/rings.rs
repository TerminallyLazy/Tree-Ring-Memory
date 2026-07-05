use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::model::{DashboardStats, RingStats};
use super::theme;

pub fn ring_color(ring: &str, warning_level: f64) -> Color {
    theme::ring_color(ring, warning_level)
}

pub fn ring_style(stats: &RingStats) -> Style {
    let mut style = Style::default().fg(ring_color(&stats.ring, stats.warning_level));
    if stats.pulse_level > 0.55 {
        style = style.add_modifier(Modifier::BOLD);
    }
    if stats.warning_level > 0.0 {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

pub fn ambient_tree_lines(dashboard: &DashboardStats, tick: u64) -> Vec<Line<'static>> {
    let phase = if tick % 8 < 4 { "*" } else { "+" };
    let empty_heartwood = RingStats::empty("heartwood");
    let empty_inner = RingStats::empty("inner");
    let empty_outer = RingStats::empty("outer");
    let empty_cambium = RingStats::empty("cambium");
    let empty_scar = RingStats::empty("scar");
    let empty_seed = RingStats::empty("seed");
    let heartwood = dashboard.ring("heartwood").unwrap_or(&empty_heartwood);
    let inner = dashboard.ring("inner").unwrap_or(&empty_inner);
    let outer = dashboard.ring("outer").unwrap_or(&empty_outer);
    let cambium = dashboard.ring("cambium").unwrap_or(&empty_cambium);
    let scar = dashboard.ring("scar").unwrap_or(&empty_scar);
    let seed = dashboard.ring("seed").unwrap_or(&empty_seed);

    vec![
        Line::from(vec![
            Span::styled("          .", theme::title()),
            Span::styled("------------------------", ring_style(cambium)),
            Span::styled(". ", theme::title()),
            Span::styled(phase.to_string(), theme::live()),
        ]),
        Line::from(vec![
            Span::styled("       .-' ", theme::title()),
            Span::styled(format!("cambium {:>3}", cambium.total), ring_style(cambium)),
            Span::styled(" fresh detail  /'-.     ", theme::title()),
        ]),
        Line::from(vec![
            Span::styled("     .'  .", theme::title()),
            Span::styled("---------------------", ring_style(outer)),
            Span::styled(". /  '.   ", theme::title()),
        ]),
        Line::from(vec![
            Span::styled("    /  .' ", theme::title()),
            Span::styled(format!("outer {:>3}", outer.total), ring_style(outer)),
            Span::styled(" detailed ring  / '.  \\  ", theme::title()),
        ]),
        Line::from(vec![
            Span::styled("   |  /  .", theme::title()),
            Span::styled("-----------------", ring_style(inner)),
            Span::styled(". /  |   | ", theme::title()),
        ]),
        Line::from(vec![
            Span::styled("   | |  | ", theme::title()),
            Span::styled(format!("inner {:>3}", inner.total), ring_style(inner)),
            Span::styled(" compressed | |  |   | ", theme::title()),
        ]),
        Line::from(vec![
            Span::styled("   | |  | ", theme::title()),
            Span::styled(
                format!("heartwood {:>3}", heartwood.total),
                ring_style(heartwood),
            ),
            Span::styled(" core | |  |   | ", theme::title()),
        ]),
        Line::from(vec![
            Span::styled("   |  \\  ' ", theme::title()),
            Span::styled(format!("scars {:>2}", scar.total), ring_style(scar)),
            Span::styled(" + ", theme::dim()),
            Span::styled(format!("seeds {:>2}", seed.total), ring_style(seed)),
            Span::styled(" ' /   |   | ", theme::title()),
        ]),
        Line::from(vec![
            Span::styled("    \\  '. ", theme::title()),
            Span::styled("evidence rings", ring_style(inner)),
            Span::styled(" .'  /   ", theme::title()),
        ]),
        Line::from(vec![
            Span::styled("      '-. '", theme::title()),
            Span::styled("===============", ring_style(cambium)),
            Span::styled("' .-'      ", theme::title()),
        ]),
        Line::from(vec![
            Span::styled("          '", theme::title()),
            Span::styled("-----------------", ring_style(cambium)),
            Span::styled("'        ", theme::title()),
        ]),
    ]
}

pub fn exploded_ring_lines(dashboard: &DashboardStats, selected_ring: usize) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(
        "EXPLODED RINGS".to_string(),
        theme::title(),
    ))];

    for (index, stats) in dashboard.rings.iter().enumerate() {
        let marker = if index == selected_ring { ">" } else { " " };
        let bar = ring_bar(stats.total, dashboard.total);
        let top_types = stats.top_event_types(2).join(", ");
        let marker_style = if index == selected_ring {
            theme::secondary_accent().add_modifier(Modifier::BOLD)
        } else {
            theme::dim()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{marker} "), marker_style),
            Span::styled(format!("{:<10}", stats.ring), ring_style(stats)),
            Span::styled(
                format!(" {:>4} ", stats.total),
                if index == selected_ring {
                    theme::selected()
                } else {
                    theme::title()
                },
            ),
            Span::styled(bar, ring_style(stats)),
            Span::styled(
                format!(
                    " conf {:.2} sal {:.2} private {}",
                    stats.average_confidence, stats.average_salience, stats.sensitive_count
                ),
                theme::dim(),
            ),
        ]));
        if !top_types.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("    top: ", theme::dim()),
                Span::styled(top_types, theme::accent()),
            ]));
        }
    }

    lines
}

fn ring_bar(count: usize, total: usize) -> String {
    let width = 16usize;
    if total == 0 {
        return ".".repeat(width);
    }
    let filled = ((count as f64 / total as f64) * width as f64).round() as usize;
    "#".repeat(filled.max(1).min(width))
        + &".".repeat(width.saturating_sub(filled.max(1).min(width)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_ring_memory_core::MemoryEvent;

    #[test]
    fn ambient_lines_include_all_core_ring_labels() {
        let mut memory = MemoryEvent::new("Durable", "lesson").unwrap();
        memory.ring = "heartwood".to_string();
        let dashboard = DashboardStats::from_memories(&[memory], None);

        let joined = ambient_tree_lines(&dashboard, 0)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("cambium"));
        assert!(joined.contains("outer"));
        assert!(joined.contains("inner"));
        assert!(joined.contains("heartwood"));
        assert!(joined.contains("scars"));
        assert!(joined.contains("seeds"));
    }
}
