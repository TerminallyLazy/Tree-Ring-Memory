#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RingMarkLayer {
    Cambium,
    Outer,
    Inner,
    Heartwood,
    Scar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RingMarkCell {
    pub ch: char,
    pub layer: Option<RingMarkLayer>,
}

impl RingMarkCell {
    fn blank() -> Self {
        Self {
            ch: ' ',
            layer: None,
        }
    }

    fn filled(ch: char, layer: RingMarkLayer) -> Self {
        Self {
            ch,
            layer: Some(layer),
        }
    }
}

pub fn ring_mark_rows(width: usize, height: usize, frame: usize) -> Vec<Vec<RingMarkCell>> {
    let width = width.max(17) | 1;
    let height = height.max(7) | 1;
    let template = if height >= 11 { LARGE_MARK } else { SMALL_MARK };
    render_template(template, width, height, frame)
}

fn glyph_for(layer: RingMarkLayer, frame: usize) -> char {
    let active = frame % 5 == pulse_index(layer);
    match (layer, active) {
        (RingMarkLayer::Cambium, true) => '@',
        (RingMarkLayer::Cambium, false) => '#',
        (RingMarkLayer::Outer, true) => '=',
        (RingMarkLayer::Outer, false) => '=',
        (RingMarkLayer::Inner, true) => '+',
        (RingMarkLayer::Inner, false) => '-',
        (RingMarkLayer::Heartwood, true) => 'O',
        (RingMarkLayer::Heartwood, false) => 'o',
        (RingMarkLayer::Scar, _) => '/',
    }
}

const LARGE_MARK: &[&str] = &[
    "          #########/",
    "       ###=======###",
    "     ##===-----===##",
    "   ##==---ooooo---==##",
    "  #==--ooooooooo--==#",
    " #==--ooooooooooo--==#",
    "  #==--ooooooooo--==#",
    "   ##==---ooooo---==##",
    "     ##===-----===##",
    "       ###/=====###",
    "          #########",
];

const SMALL_MARK: &[&str] = &[
    "      #######/",
    "   ##=======##",
    " ##==-----==##",
    "#==--ooooo--==#",
    " ##==-----==##",
    "   ##/====##",
    "      #####",
];

fn render_template(
    template: &[&str],
    width: usize,
    height: usize,
    frame: usize,
) -> Vec<Vec<RingMarkCell>> {
    let template_width = template.iter().map(|row| row.len()).max().unwrap_or(0);
    let x_offset = width.saturating_sub(template_width) / 2;
    let y_offset = height.saturating_sub(template.len()) / 2;
    let mut rows = vec![vec![RingMarkCell::blank(); width]; height];

    for (template_y, source_row) in template.iter().enumerate() {
        let Some(target_row) = rows.get_mut(y_offset + template_y) else {
            continue;
        };
        for (template_x, ch) in source_row.chars().enumerate() {
            let target_x = x_offset + template_x;
            if target_x >= width {
                continue;
            }
            target_row[target_x] = cell_from_template_char(ch, frame);
        }
    }

    rows
}

fn cell_from_template_char(ch: char, frame: usize) -> RingMarkCell {
    let layer = match ch {
        '#' | '@' => RingMarkLayer::Cambium,
        '=' => RingMarkLayer::Outer,
        '-' | '+' => RingMarkLayer::Inner,
        'o' | 'O' => RingMarkLayer::Heartwood,
        '/' => RingMarkLayer::Scar,
        _ => return RingMarkCell::blank(),
    };
    RingMarkCell::filled(glyph_for(layer, frame), layer)
}

pub fn pulse_index(layer: RingMarkLayer) -> usize {
    match layer {
        RingMarkLayer::Cambium => 0,
        RingMarkLayer::Outer => 1,
        RingMarkLayer::Inner => 2,
        RingMarkLayer::Heartwood => 3,
        RingMarkLayer::Scar => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_contains_distinct_ring_layers_and_scar() {
        let rendered = ring_mark_rows(29, 11, 0)
            .into_iter()
            .map(|row| row.into_iter().map(|cell| cell.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains('#') || rendered.contains('@'));
        assert!(rendered.contains('='));
        assert!(rendered.contains('-') || rendered.contains('+'));
        assert!(rendered.contains('o') || rendered.contains('O'));
        assert!(rendered.contains('/'));
    }

    #[test]
    fn mark_keeps_requested_odd_dimensions() {
        let rows = ring_mark_rows(23, 7, 1);

        assert_eq!(rows.len(), 7);
        assert!(rows.iter().all(|row| row.len() == 23));
    }
}
