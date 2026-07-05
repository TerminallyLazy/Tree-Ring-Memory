use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::model::{DashboardStats, RingStats};
use super::theme;

const SYMBOL_WIDTH: usize = 28;
const SYMBOL_HEIGHT: usize = 18;

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
    let labels = [
        label("C", cambium),
        label("O", outer),
        label("I", inner),
        label("H", heartwood),
        label("!", scar),
        label("?", seed),
    ];

    let mut lines = vec![Line::from(vec![
        Span::styled("live ", theme::live()),
        Span::styled(
            spin.to_string(),
            theme::secondary_accent().add_modifier(Modifier::BOLD),
        ),
        Span::styled(" total ", theme::dim()),
        Span::styled(format!("{:>3}", dashboard.total), theme::title()),
    ])];

    let symbol_rows = ring_symbol_lines(dashboard, tick);
    for (index, mut line) in symbol_rows.into_iter().enumerate() {
        line.spans.push(Span::raw("  "));
        if let Some(label) = labels.get(index) {
            line.spans.extend(label.clone());
        }
        lines.push(line);
    }

    lines.push(Line::from(vec![
        Span::styled("store-watch ", theme::accent()),
        Span::styled("+ event-stream", theme::secondary_accent()),
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

fn label(prefix: &'static str, stats: &RingStats) -> Vec<Span<'static>> {
    vec![
        Span::styled(prefix, ring_style(stats)),
        Span::styled(format!("{:>3}", stats.total), ring_style(stats)),
    ]
}

fn ring_symbol_lines(dashboard: &DashboardStats, tick: u64) -> Vec<Line<'static>> {
    (0..SYMBOL_HEIGHT)
        .step_by(2)
        .map(|top_y| {
            let mut spans = Vec::new();
            for x in 0..SYMBOL_WIDTH {
                let top = ring_pixel_color(dashboard, tick, x, top_y);
                let bottom = ring_pixel_color(dashboard, tick, x, top_y + 1);
                spans.push(pixel_span(top, bottom));
            }
            Line::from(spans)
        })
        .collect()
}

fn pixel_span(top: Option<Color>, bottom: Option<Color>) -> Span<'static> {
    match (top, bottom) {
        (Some(top), Some(bottom)) => Span::styled("▀", Style::default().fg(top).bg(bottom)),
        (Some(top), None) => Span::styled("▀", Style::default().fg(top)),
        (None, Some(bottom)) => Span::styled("▄", Style::default().fg(bottom)),
        (None, None) => Span::raw(" "),
    }
}

fn ring_pixel_color(dashboard: &DashboardStats, tick: u64, x: usize, y: usize) -> Option<Color> {
    let cx = (SYMBOL_WIDTH as f64 - 1.0) / 2.0;
    let cy = (SYMBOL_HEIGHT as f64 - 1.0) / 2.0;
    let nx = (x as f64 - cx) / (SYMBOL_WIDTH as f64 * 0.42);
    let ny = (y as f64 - cy) / (SYMBOL_HEIGHT as f64 * 0.42);
    let radius = (nx * nx + ny * ny).sqrt();
    if radius > 1.0 {
        return None;
    }

    let mut angle = (-ny).atan2(nx).to_degrees();
    if angle < 0.0 {
        angle += 360.0;
    }
    let wedge_gap =
        ((34.0..=57.0).contains(&angle) || (214.0..=237.0).contains(&angle)) && radius > 0.32;
    if wedge_gap {
        let scar = dashboard.ring("scar");
        if scar.map(|stats| stats.total).unwrap_or_default() > 0 && radius > 0.44 {
            return Some(animated_color("scar", scar, tick, 4));
        }
        return None;
    }

    let boundary = [0.24, 0.42, 0.61, 0.80, 0.94]
        .iter()
        .any(|ring_radius| (radius - ring_radius).abs() < 0.025);
    if boundary {
        return Some(theme::NAVY);
    }

    let ring = if radius <= 0.24 {
        "heartwood"
    } else if radius <= 0.42 {
        "inner"
    } else if radius <= 0.61 {
        "outer"
    } else if radius <= 0.80 {
        "cambium"
    } else {
        "cambium"
    };
    Some(animated_color(
        ring,
        dashboard.ring(ring),
        tick,
        ring_offset(ring),
    ))
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
        assert!(joined.contains("C  1"));
        assert!(joined.contains("H  1"));
        assert!(joined.contains("store-watch"));
        assert!(joined.contains("event-stream"));
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
