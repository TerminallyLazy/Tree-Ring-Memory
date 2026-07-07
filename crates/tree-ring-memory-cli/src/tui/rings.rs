use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::ring_mark::{
    ring_mark_rows_with_activity, RingMarkActivity, RingMarkCell, RingMarkLayer,
};

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

pub fn ambient_corner_lines(dashboard: &DashboardStats, tick: u64) -> Vec<Line<'static>> {
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
    let spin = ["*", "+", ".", "+"][(tick as usize / 2) % 4];

    let mut lines = vec![Line::from(vec![
        Span::styled("live ", theme::live()),
        Span::styled(
            spin.to_string(),
            theme::secondary_accent().add_modifier(Modifier::BOLD),
        ),
        Span::styled(" total ", theme::dim()),
        Span::styled(format!("{:>3}", dashboard.total), theme::title()),
    ])];
    lines.extend(terminal_ring_lines(
        cambium, outer, inner, heartwood, scar, tick,
    ));
    lines.push(count_line([
        ("C", cambium),
        ("O", outer),
        ("I", inner),
        ("H", heartwood),
        ("!", scar),
        ("?", seed),
    ]));
    lines
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

fn ring_offset(ring: &str) -> u64 {
    match ring {
        "heartwood" => 0,
        "inner" => 1,
        "outer" => 2,
        "cambium" => 3,
        "scar" => 4,
        "seed" => 5,
        _ => 0,
    }
}

fn terminal_ring_lines(
    cambium: &RingStats,
    outer: &RingStats,
    inner: &RingStats,
    heartwood: &RingStats,
    scar: &RingStats,
    tick: u64,
) -> Vec<Line<'static>> {
    ring_mark_rows_with_activity(
        31,
        10,
        tick as usize,
        ring_activity(cambium, outer, inner, heartwood, scar),
    )
    .into_iter()
    .map(|row| ring_mark_line(row, cambium, outer, inner, heartwood, scar, tick))
    .collect()
}

fn ring_activity(
    cambium: &RingStats,
    outer: &RingStats,
    inner: &RingStats,
    heartwood: &RingStats,
    scar: &RingStats,
) -> RingMarkActivity {
    RingMarkActivity {
        cambium: activity_level(cambium),
        outer: activity_level(outer),
        inner: activity_level(inner),
        heartwood: activity_level(heartwood),
        scar: activity_level(scar),
    }
}

fn activity_level(stats: &RingStats) -> f64 {
    let baseline = if stats.total > 0 { 0.16 } else { 0.03 };
    (baseline + stats.pulse_level * 0.84).clamp(0.0, 1.0)
}

fn terminal_ring_style(stats: &RingStats, tick: u64, brightness: u8) -> Style {
    Style::default()
        .fg(lit_color(stats, tick, brightness))
        .add_modifier(if stats.total > 0 && stats.pulse_level > 0.35 {
            Modifier::BOLD
        } else {
            Modifier::empty()
        })
}

fn ring_mark_line(
    row: Vec<RingMarkCell>,
    cambium: &RingStats,
    outer: &RingStats,
    inner: &RingStats,
    heartwood: &RingStats,
    scar: &RingStats,
    tick: u64,
) -> Line<'static> {
    Line::from(
        row.into_iter()
            .map(|cell| {
                let text = cell.ch.to_string();
                Span::styled(
                    text,
                    terminal_cell_style(cell, cambium, outer, inner, heartwood, scar, tick),
                )
            })
            .collect::<Vec<_>>(),
    )
}

fn terminal_cell_style(
    cell: RingMarkCell,
    cambium: &RingStats,
    outer: &RingStats,
    inner: &RingStats,
    heartwood: &RingStats,
    scar: &RingStats,
    tick: u64,
) -> Style {
    match (cell.upper_layer, cell.lower_layer) {
        (None, None) => Style::default(),
        (Some(upper), Some(lower)) if upper == lower => terminal_ring_style(
            layer_stats(upper, cambium, outer, inner, heartwood, scar),
            tick,
            cell.brightness,
        ),
        (Some(upper), Some(lower)) => {
            let upper_stats = layer_stats(upper, cambium, outer, inner, heartwood, scar);
            let lower_stats = layer_stats(lower, cambium, outer, inner, heartwood, scar);
            let mut style = Style::default()
                .fg(lit_color(upper_stats, tick, cell.brightness))
                .bg(animated_ring_color(lower_stats, tick));
            if upper_stats.pulse_level > 0.35 || lower_stats.pulse_level > 0.35 {
                style = style.add_modifier(Modifier::BOLD);
            }
            style
        }
        (Some(upper), None) => terminal_ring_style(
            layer_stats(upper, cambium, outer, inner, heartwood, scar),
            tick,
            cell.brightness,
        ),
        (None, Some(lower)) => terminal_ring_style(
            layer_stats(lower, cambium, outer, inner, heartwood, scar),
            tick,
            cell.brightness,
        ),
    }
}

fn animated_ring_color(stats: &RingStats, tick: u64) -> Color {
    animated_color(&stats.ring, Some(stats), tick, ring_offset(&stats.ring))
}

fn lit_color(stats: &RingStats, tick: u64, brightness: u8) -> Color {
    let base = animated_ring_color(stats, tick);
    let amount = brightness as f64 / 255.0;
    if amount < 0.48 {
        dim_color(base, 0.38 + amount * 0.85)
    } else {
        brighten_color(base, (amount - 0.48) * 0.46)
    }
}

fn layer_stats<'a>(
    layer: RingMarkLayer,
    cambium: &'a RingStats,
    outer: &'a RingStats,
    inner: &'a RingStats,
    heartwood: &'a RingStats,
    scar: &'a RingStats,
) -> &'a RingStats {
    match layer {
        RingMarkLayer::Cambium => cambium,
        RingMarkLayer::Outer => outer,
        RingMarkLayer::Inner => inner,
        RingMarkLayer::Heartwood => heartwood,
        RingMarkLayer::Scar => scar,
    }
}

fn count_line<const N: usize>(items: [(&'static str, &RingStats); N]) -> Line<'static> {
    let mut spans = Vec::new();
    spans.push(Span::raw(" "));
    for (index, (label, stats)) in items.into_iter().enumerate() {
        if label.is_empty() {
            continue;
        }
        if index > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(label, ring_style(stats)));
        spans.push(Span::styled(stats.total.to_string(), ring_style(stats)));
    }
    Line::from(spans)
}

fn animated_color(ring: &str, stats: Option<&RingStats>, tick: u64, offset: u64) -> Color {
    let warning_level = stats.map(|stats| stats.warning_level).unwrap_or_default();
    let base = theme::ring_color(ring, warning_level);
    let Some(stats) = stats else {
        return dim_color(base, 0.36);
    };
    if stats.total == 0 {
        return dim_color(base, 0.34);
    }
    if stats.pulse_level > 0.05 && (tick + offset) % 6 < 3 {
        return brighten_color(base, 0.34 + (stats.pulse_level * 0.18));
    }
    if (tick + offset) % 18 < 3 {
        return brighten_color(base, 0.18);
    }
    base
}

fn brighten_color(color: Color, amount: f64) -> Color {
    match color {
        Color::Rgb(red, green, blue) => Color::Rgb(
            brighten_channel(red, amount),
            brighten_channel(green, amount),
            brighten_channel(blue, amount),
        ),
        other => other,
    }
}

fn brighten_channel(value: u8, amount: f64) -> u8 {
    let value = value as f64;
    (value + ((255.0 - value) * amount.clamp(0.0, 1.0))).round() as u8
}

fn dim_color(color: Color, amount: f64) -> Color {
    match color {
        Color::Rgb(red, green, blue) => Color::Rgb(
            dim_channel(red, amount),
            dim_channel(green, amount),
            dim_channel(blue, amount),
        ),
        other => other,
    }
}

fn dim_channel(value: u8, amount: f64) -> u8 {
    ((value as f64) * amount.clamp(0.0, 1.0)).round() as u8
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

    #[test]
    fn ambient_corner_lines_include_live_ring_counts() {
        let mut cambium = MemoryEvent::new("Fresh detail", "lesson").unwrap();
        cambium.ring = "cambium".to_string();
        let mut heartwood = MemoryEvent::new("Durable truth", "decision").unwrap();
        heartwood.ring = "heartwood".to_string();
        let dashboard = DashboardStats::from_memories(&[cambium, heartwood], None);

        let joined = ambient_corner_lines(&dashboard, 0)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("live"));
        assert!(joined.contains("total   2"));
        assert!(joined.contains("C1"));
        assert!(joined.contains("H1"));
        assert!(joined.contains("!0"));
        assert!(joined.contains("?0"));
    }

    #[test]
    fn ambient_corner_lines_animate_with_tick() {
        let mut cambium = MemoryEvent::new("Fresh detail", "lesson").unwrap();
        cambium.ring = "cambium".to_string();
        let mut first = DashboardStats::from_memories(&[], None);
        let dashboard = DashboardStats::from_memories(&[cambium], Some(&first));
        first = dashboard.clone();

        let early = ambient_corner_lines(&first, 0)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let later = ambient_corner_lines(&first, 2)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert_ne!(early, later);
    }
}
