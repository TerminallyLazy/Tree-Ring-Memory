use std::f64::consts::PI;

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
    pub upper_layer: Option<RingMarkLayer>,
    pub lower_layer: Option<RingMarkLayer>,
    pub brightness: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RingMarkActivity {
    pub cambium: f64,
    pub outer: f64,
    pub inner: f64,
    pub heartwood: f64,
    pub scar: f64,
}

impl RingMarkActivity {
    pub fn layer(self, layer: RingMarkLayer) -> f64 {
        match layer {
            RingMarkLayer::Cambium => self.cambium,
            RingMarkLayer::Outer => self.outer,
            RingMarkLayer::Inner => self.inner,
            RingMarkLayer::Heartwood => self.heartwood,
            RingMarkLayer::Scar => self.scar,
        }
        .clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RingMarkFrame {
    width: usize,
    height: usize,
    pixels: Vec<RingMarkCell>,
}

impl RingMarkFrame {
    pub fn with_activity(
        width_cells: usize,
        height_cells: usize,
        frame: usize,
        activity: RingMarkActivity,
    ) -> Self {
        let width = width_cells.max(23) | 1;
        let height = height_cells.max(7);
        let pixels = sample_tree_rings(width, height, frame, activity);

        Self {
            width,
            height,
            pixels,
        }
    }

    #[cfg(test)]
    fn layer_at(&self, col: usize, row: usize) -> Option<RingMarkLayer> {
        if col >= self.width || row >= self.height {
            return None;
        }
        self.pixels[row * self.width + col].layer
    }

    pub fn half_block_rows(&self) -> Vec<Vec<RingMarkCell>> {
        (0..self.height)
            .map(|row| {
                (0..self.width)
                    .map(|col| self.pixels[row * self.width + col])
                    .collect()
            })
            .collect()
    }
}

impl RingMarkCell {
    fn blank() -> Self {
        Self {
            ch: ' ',
            layer: None,
            upper_layer: None,
            lower_layer: None,
            brightness: 0,
        }
    }

    fn solid(ch: char, layer: RingMarkLayer, brightness: u8) -> Self {
        Self {
            ch,
            layer: Some(layer),
            upper_layer: Some(layer),
            lower_layer: Some(layer),
            brightness,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Band {
    layer: RingMarkLayer,
    radius: f64,
    half_width: f64,
    spin_direction: f64,
    spin_speed: f64,
    spin_offset: f64,
}

const BANDS: &[Band] = &[
    Band {
        layer: RingMarkLayer::Heartwood,
        radius: 0.15,
        half_width: 0.075,
        spin_direction: 1.0,
        spin_speed: 1.00,
        spin_offset: 0.20,
    },
    Band {
        layer: RingMarkLayer::Inner,
        radius: 0.33,
        half_width: 0.040,
        spin_direction: -1.0,
        spin_speed: 1.22,
        spin_offset: 1.10,
    },
    Band {
        layer: RingMarkLayer::Inner,
        radius: 0.47,
        half_width: 0.034,
        spin_direction: -1.0,
        spin_speed: 1.08,
        spin_offset: 2.00,
    },
    Band {
        layer: RingMarkLayer::Outer,
        radius: 0.61,
        half_width: 0.034,
        spin_direction: 1.0,
        spin_speed: 0.92,
        spin_offset: 2.80,
    },
    Band {
        layer: RingMarkLayer::Outer,
        radius: 0.75,
        half_width: 0.031,
        spin_direction: 1.0,
        spin_speed: 0.78,
        spin_offset: 3.60,
    },
    Band {
        layer: RingMarkLayer::Cambium,
        radius: 0.91,
        half_width: 0.046,
        spin_direction: -1.0,
        spin_speed: 0.70,
        spin_offset: 4.40,
    },
];

pub fn ring_mark_rows_with_activity(
    width: usize,
    height: usize,
    frame: usize,
    activity: RingMarkActivity,
) -> Vec<Vec<RingMarkCell>> {
    RingMarkFrame::with_activity(width, height, frame, activity).half_block_rows()
}

#[derive(Clone, Copy, Debug)]
struct SubPixelHit {
    layer: RingMarkLayer,
    brightness: u8,
}

#[derive(Clone, Copy, Debug)]
struct SamplingGeometry {
    width: usize,
    height: usize,
    phase: f64,
}

#[derive(Clone, Copy, Debug)]
struct SubPixelSample {
    col: usize,
    row: usize,
    sub_x: usize,
    sub_y: usize,
}

fn sample_tree_rings(
    width: usize,
    height: usize,
    frame: usize,
    activity: RingMarkActivity,
) -> Vec<RingMarkCell> {
    let geometry = SamplingGeometry {
        width,
        height,
        phase: frame as f64 * 0.22,
    };
    let mut cells = Vec::with_capacity(width * height);

    for row in 0..height {
        for col in 0..width {
            cells.push(sample_cell(col, row, geometry, activity));
        }
    }

    cells
}

fn sample_cell(
    col: usize,
    row: usize,
    geometry: SamplingGeometry,
    activity: RingMarkActivity,
) -> RingMarkCell {
    let mut pattern = 0u8;
    let mut brightness_sum = 0usize;
    let mut layer_counts = [0usize; 5];
    let mut hits = 0usize;
    for sub_y in 0..2 {
        for sub_x in 0..2 {
            let sample = SubPixelSample {
                col,
                row,
                sub_x,
                sub_y,
            };
            if let Some(hit) = sample_subpixel(sample, geometry, activity) {
                pattern |= quadrant_bit(sub_x, sub_y);
                brightness_sum += hit.brightness as usize;
                layer_counts[pulse_index(hit.layer)] += 1;
                hits += 1;
            }
        }
    }

    if hits == 0 {
        return RingMarkCell::blank();
    }

    RingMarkCell::solid(
        quadrant_char(pattern),
        dominant_layer(layer_counts),
        (brightness_sum / hits) as u8,
    )
}

fn sample_subpixel(
    sample: SubPixelSample,
    geometry: SamplingGeometry,
    activity: RingMarkActivity,
) -> Option<SubPixelHit> {
    let x = normalized_x(sample.col, sample.sub_x, geometry.width);
    let y = normalized_y(sample.row, sample.sub_y, geometry.height);
    let radius = (x * x + y * y).sqrt();
    if radius > 1.06 {
        return None;
    }
    let angle = normalize_angle(y.atan2(x));

    if let Some(hit) = scar_hit(
        x,
        y,
        radius,
        geometry.phase,
        activity.layer(RingMarkLayer::Scar),
    ) {
        return Some(hit);
    }

    let mut best: Option<(f64, SubPixelHit)> = None;
    for band in BANDS {
        let layer_activity = activity.layer(band.layer);
        let center = animated_radius(*band, geometry.phase, layer_activity);
        let half_width = band.half_width + layer_activity * 0.016;
        let distance = (radius - center).abs();
        if distance > half_width {
            continue;
        }
        let brightness = band_brightness(
            *band,
            angle,
            radius,
            geometry.phase,
            layer_activity,
            distance,
        );
        let hit = SubPixelHit {
            layer: band.layer,
            brightness,
        };
        if best
            .map(|(best_distance, _)| distance < best_distance)
            .unwrap_or(true)
        {
            best = Some((distance, hit));
        }
    }

    best.map(|(_, hit)| hit)
}

fn normalized_x(col: usize, sub_x: usize, width: usize) -> f64 {
    let cell = col as f64 + (sub_x as f64 + 0.5) / 2.0;
    let center = width as f64 * 0.5;
    (cell - center) / (width as f64 * 0.30)
}

fn normalized_y(row: usize, sub_y: usize, height: usize) -> f64 {
    let cell = row as f64 + (sub_y as f64 + 0.5) / 2.0;
    let center = height as f64 * 0.5;
    (cell - center) / (height as f64 * 0.52)
}

fn animated_radius(band: Band, phase: f64, layer_activity: f64) -> f64 {
    let layer_phase = pulse_index(band.layer) as f64 * 0.81;
    let breathing = (phase * 1.35 + layer_phase).sin() * 0.007;
    let pulse = layer_activity * (0.020 + 0.006 * (phase * 2.1 + layer_phase).sin().abs());
    (band.radius + breathing + pulse).min(1.02)
}

fn band_brightness(
    band: Band,
    angle: f64,
    radius: f64,
    phase: f64,
    layer_activity: f64,
    distance: f64,
) -> u8 {
    let spin =
        normalize_angle(angle + band.spin_direction * phase * band.spin_speed + band.spin_offset);
    let moving_light = angular_glow(spin, 0.0, 0.19).max(angular_glow(spin, PI * 1.27, 0.13));
    let grain = ((angle * 11.0 + radius * 17.0 + band.spin_direction * phase * 0.75).sin() * 0.5
        + 0.5)
        * 0.18;
    let pulse = layer_activity * (0.28 + 0.22 * (phase * 2.8 + band.spin_offset).sin().max(0.0));
    let edge = 1.0 - (distance / band.half_width.max(0.001)).clamp(0.0, 1.0);
    let value = 0.30 + edge * 0.18 + grain + moving_light * (0.34 + layer_activity * 0.20) + pulse;
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn scar_hit(x: f64, y: f64, radius: f64, phase: f64, scar_activity: f64) -> Option<SubPixelHit> {
    if !(0.20..=0.88).contains(&radius) {
        return None;
    }

    let upper = (y + x * 0.62 + 0.02).abs();
    let lower = (y - x * 0.42 - 0.22).abs();
    let width = 0.026 + scar_activity * 0.020;
    let in_upper = upper < width && (-0.48..=0.54).contains(&x);
    let in_lower = lower < width * 0.85 && (-0.16..=0.48).contains(&x);
    if !in_upper && !in_lower {
        return None;
    }

    let shimmer = ((phase * 4.2 + x * 9.0 - y * 6.0).sin() * 0.5 + 0.5) * 0.24;
    let value = 0.58 + scar_activity * 0.28 + shimmer;
    Some(SubPixelHit {
        layer: RingMarkLayer::Scar,
        brightness: (value.clamp(0.0, 1.0) * 255.0).round() as u8,
    })
}

fn angular_glow(angle: f64, target: f64, width: f64) -> f64 {
    let distance = angular_distance(angle, target);
    if distance >= width {
        0.0
    } else {
        let t = 1.0 - distance / width;
        t * t
    }
}

fn dominant_layer(layer_counts: [usize; 5]) -> RingMarkLayer {
    let index = layer_counts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| **count)
        .map(|(index, _)| index)
        .unwrap_or(0);
    match index {
        0 => RingMarkLayer::Cambium,
        1 => RingMarkLayer::Outer,
        2 => RingMarkLayer::Inner,
        3 => RingMarkLayer::Heartwood,
        4 => RingMarkLayer::Scar,
        _ => RingMarkLayer::Cambium,
    }
}

fn quadrant_bit(sub_x: usize, sub_y: usize) -> u8 {
    match (sub_x, sub_y) {
        (0, 0) => 0x01,
        (1, 0) => 0x02,
        (0, 1) => 0x04,
        (1, 1) => 0x08,
        _ => 0,
    }
}

fn quadrant_char(pattern: u8) -> char {
    match pattern {
        0x01 => '▘',
        0x02 => '▝',
        0x03 => '▀',
        0x04 => '▖',
        0x05 => '▌',
        0x06 => '▞',
        0x07 => '▛',
        0x08 => '▗',
        0x09 => '▚',
        0x0a => '▐',
        0x0b => '▜',
        0x0c => '▄',
        0x0d => '▙',
        0x0e => '▟',
        0x0f => '█',
        _ => ' ',
    }
}

fn angular_distance(angle: f64, target: f64) -> f64 {
    let mut diff = (angle - target).abs() % (PI * 2.0);
    if diff > PI {
        diff = PI * 2.0 - diff;
    }
    diff
}

fn normalize_angle(angle: f64) -> f64 {
    let mut value = angle % (PI * 2.0);
    if value < 0.0 {
        value += PI * 2.0;
    }
    value
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
        let frame = RingMarkFrame::with_activity(31, 12, 0, RingMarkActivity::default());
        for layer in [
            RingMarkLayer::Cambium,
            RingMarkLayer::Outer,
            RingMarkLayer::Inner,
            RingMarkLayer::Heartwood,
            RingMarkLayer::Scar,
        ] {
            assert!(
                frame.pixels.iter().any(|pixel| pixel.layer == Some(layer)),
                "missing {layer:?}"
            );
        }
    }

    #[test]
    fn frame_is_backend_independent_layer_data() {
        let frame = RingMarkFrame::with_activity(31, 12, 0, RingMarkActivity::default());

        assert_eq!(frame.width, 31);
        assert_eq!(frame.height, 12);
        assert_eq!(frame.layer_at(usize::MAX, 0), None);
        assert_eq!(frame.layer_at(0, usize::MAX), None);
        assert_eq!(frame.half_block_rows().len(), 12);
    }

    #[test]
    fn mark_uses_smooth_quadrant_ring_cells_instead_of_noisy_ascii() {
        let rendered = ring_mark_rows_with_activity(31, 12, 0, RingMarkActivity::default())
            .into_iter()
            .flatten()
            .filter(|cell| cell.layer.is_some())
            .map(|cell| cell.ch)
            .collect::<String>();

        assert!(rendered.chars().all(is_quadrant_cell));
        assert!(rendered.contains('█') || rendered.contains('▀') || rendered.contains('▄'));
        assert!(!rendered.contains('('));
        assert!(!rendered.contains(')'));
        assert!(!rendered.contains('/'));
        assert!(!rendered.contains('\\'));
        assert!(!rendered.contains('@'));
        assert!(!rendered.contains('#'));
        assert!(!rendered.contains('%'));
    }

    #[test]
    fn pulse_frame_keeps_ring_geometry_stable() {
        let first = ring_mark_rows_with_activity(31, 12, 0, RingMarkActivity::default());
        let second = ring_mark_rows_with_activity(31, 12, 1, RingMarkActivity::default());

        assert_eq!(first.len(), second.len());
        assert!(first.iter().zip(&second).all(|(a, b)| a.len() == b.len()));
        assert_ne!(first, second);
    }

    #[test]
    fn live_activity_expands_matching_ring_geometry() {
        let calm = RingMarkFrame::with_activity(31, 12, 0, RingMarkActivity::default());
        let live = RingMarkFrame::with_activity(
            31,
            12,
            0,
            RingMarkActivity {
                cambium: 1.0,
                ..RingMarkActivity::default()
            },
        );

        let calm_cambium = calm
            .pixels
            .iter()
            .filter(|pixel| pixel.layer == Some(RingMarkLayer::Cambium))
            .count();
        let live_cambium = live
            .pixels
            .iter()
            .filter(|pixel| pixel.layer == Some(RingMarkLayer::Cambium))
            .count();

        assert!(live_cambium > calm_cambium);
    }

    #[test]
    fn mark_keeps_requested_odd_dimensions() {
        let rows = ring_mark_rows_with_activity(23, 7, 1, RingMarkActivity::default());

        assert_eq!(rows.len(), 7);
        assert!(rows.iter().all(|row| row.len() == 23));
    }

    fn is_quadrant_cell(ch: char) -> bool {
        matches!(
            ch,
            '▘' | '▝'
                | '▀'
                | '▖'
                | '▌'
                | '▞'
                | '▛'
                | '▗'
                | '▚'
                | '▐'
                | '▜'
                | '▄'
                | '▙'
                | '▟'
                | '█'
        )
    }
}
