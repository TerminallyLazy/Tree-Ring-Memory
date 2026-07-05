use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};

pub const NAVY: Color = Color::Rgb(0, 31, 52);
pub const TEAL: Color = Color::Rgb(22, 156, 166);
pub const PINK: Color = Color::Rgb(239, 65, 103);
pub const ORANGE: Color = Color::Rgb(255, 125, 34);
pub const YELLOW: Color = Color::Rgb(255, 194, 69);
pub const CREAM: Color = Color::Rgb(255, 245, 213);
pub const CORAL: Color = Color::Rgb(255, 101, 83);
pub const MUTED: Color = Color::Rgb(121, 135, 145);

pub fn title() -> Style {
    Style::default().fg(CREAM).add_modifier(Modifier::BOLD)
}

pub fn brand() -> Style {
    Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)
}

pub fn accent() -> Style {
    Style::default().fg(TEAL)
}

pub fn secondary_accent() -> Style {
    Style::default().fg(PINK)
}

pub fn warning() -> Style {
    Style::default().fg(CORAL).add_modifier(Modifier::BOLD)
}

pub fn dim() -> Style {
    Style::default().fg(MUTED)
}

pub fn selected() -> Style {
    Style::default()
        .fg(CREAM)
        .bg(NAVY)
        .add_modifier(Modifier::BOLD)
}

pub fn live() -> Style {
    Style::default().fg(TEAL).add_modifier(Modifier::BOLD)
}

pub fn panel<'a>(title: impl Into<String>) -> Block<'a> {
    Block::default()
        .title(title.into())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(TEAL))
        .title_style(title_style())
}

pub fn plain_panel<'a>() -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(TEAL))
}

fn title_style() -> Style {
    Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)
}

pub fn ring_color(ring: &str, warning_level: f64) -> Color {
    if warning_level > 0.75 {
        return CORAL;
    }
    match ring {
        "cambium" => TEAL,
        "outer" => PINK,
        "inner" => ORANGE,
        "heartwood" => YELLOW,
        "scar" => CORAL,
        "seed" => Color::Rgb(82, 210, 202),
        _ => MUTED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_core_rings_to_brand_palette() {
        assert_eq!(ring_color("cambium", 0.0), TEAL);
        assert_eq!(ring_color("outer", 0.0), PINK);
        assert_eq!(ring_color("inner", 0.0), ORANGE);
        assert_eq!(ring_color("heartwood", 0.0), YELLOW);
        assert_eq!(ring_color("scar", 0.0), CORAL);
        assert_eq!(ring_color("seed", 0.0), Color::Rgb(82, 210, 202));
        assert_eq!(ring_color("unknown", 0.0), MUTED);
    }

    #[test]
    fn warning_overrides_ring_color() {
        assert_eq!(ring_color("heartwood", 1.0), CORAL);
    }
}
