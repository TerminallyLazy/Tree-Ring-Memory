use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::model::{DashboardStats, RingStats};

pub fn ring_color(ring: &str, warning_level: f64) -> Color {
    if warning_level > 0.75 {
        return Color::LightRed;
    }
    match ring {
        "cambium" => Color::LightYellow,
        "outer" => Color::LightMagenta,
        "inner" => Color::Cyan,
        "heartwood" => Color::Yellow,
        "scar" => Color::Red,
        "seed" => Color::LightCyan,
        _ => Color::Gray,
    }
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
    let fallback = RingStats::empty("unknown");
    let heartwood = dashboard.ring("heartwood").unwrap_or(&fallback);
    let inner = dashboard.ring("inner").unwrap_or(&fallback);
    let outer = dashboard.ring("outer").unwrap_or(&fallback);
    let cambium = dashboard.ring("cambium").unwrap_or(&fallback);
    let scar = dashboard.ring("scar").unwrap_or(&fallback);
    let seed = dashboard.ring("seed").unwrap_or(&fallback);

    vec![
        Line::from(Span::styled(
            format!("        .-=================-.        {phase}"),
            ring_style(cambium),
        )),
        Line::from(Span::styled(
            format!("     .-'   cambium {:>5}   '-.", cambium.total),
            ring_style(cambium),
        )),
        Line::from(Span::styled(
            format!("   .'   .--- outer {:>5} ---.   '.", outer.total),
            ring_style(outer),
        )),
        Line::from(Span::styled(
            format!("  /   .'   .-- inner {:>5} --.   '.  \\", inner.total),
            ring_style(inner),
        )),
        Line::from(vec![
            Span::styled(" |   /   .'", ring_style(inner)),
            Span::styled(
                format!(" heartwood {:>5} ", heartwood.total),
                ring_style(heartwood),
            ),
            Span::styled("'.   \\   |", ring_style(inner)),
        ]),
        Line::from(Span::styled(
            format!(
                "  \\   '.   scars {:>4} seeds {:>4} .'   /",
                scar.total, seed.total
            ),
            if scar.total > 0 {
                ring_style(scar)
            } else {
                ring_style(seed)
            },
        )),
        Line::from(Span::styled(
            "   '.   '---.          .---'   .'".to_string(),
            ring_style(outer),
        )),
        Line::from(Span::styled(
            "     '-.       '------'       .-'".to_string(),
            ring_style(cambium),
        )),
        Line::from(Span::styled(
            "        '==================='".to_string(),
            ring_style(cambium),
        )),
    ]
}

pub fn exploded_ring_lines(dashboard: &DashboardStats, selected_ring: usize) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(
        "EXPLODED RINGS".to_string(),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ))];

    for (index, stats) in dashboard.rings.iter().enumerate() {
        let marker = if index == selected_ring { ">" } else { " " };
        let bar = ring_bar(stats.total, dashboard.total);
        let top_types = stats.top_event_types(2).join(", ");
        lines.push(Line::from(vec![
            Span::raw(format!("{marker} ")),
            Span::styled(format!("{:<10}", stats.ring), ring_style(stats)),
            Span::raw(format!(" {:>4} ", stats.total)),
            Span::styled(bar, ring_style(stats)),
            Span::raw(format!(
                " conf {:.2} sal {:.2} private {}",
                stats.average_confidence, stats.average_salience, stats.sensitive_count
            )),
        ]));
        if !top_types.is_empty() {
            lines.push(Line::from(Span::raw(format!("    top: {top_types}"))));
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
    }
}
