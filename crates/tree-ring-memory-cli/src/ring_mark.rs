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
    pixels: Vec<Option<RingMarkLayer>>,
}

impl RingMarkFrame {
    pub fn with_activity(
        width_cells: usize,
        height_cells: usize,
        frame: usize,
        activity: RingMarkActivity,
    ) -> Self {
        let width = width_cells.max(17) | 1;
        let height_cells = height_cells.max(7) | 1;
        let height = height_cells * 2;
        let center_x = (width as f64 - 1.0) / 2.0;
        let center_y = (height as f64 - 1.0) / 2.0;
        let radius = width.min(height) as f64 * 0.47;
        let phase = frame as f64 * 0.28;
        let perspective = 0.86 + phase.sin() * 0.050;
        let shear = phase.cos() * 0.055;
        let highlight_angle = normalize_angle(phase * 0.84 - 0.40);
        let pixels = (0..height)
            .flat_map(|row| {
                (0..width).map(move |col| {
                    sample_pixel(
                        PixelSample {
                            col,
                            row,
                            center_x,
                            center_y,
                            radius_scale: radius,
                            perspective,
                            shear,
                            highlight_angle,
                        },
                        frame,
                        activity,
                    )
                })
            })
            .collect();

        Self {
            width,
            height,
            pixels,
        }
    }

    pub fn layer_at(&self, col: usize, row: usize) -> Option<RingMarkLayer> {
        if col >= self.width || row >= self.height {
            return None;
        }
        self.pixels[row * self.width + col]
    }

    pub fn half_block_rows(&self) -> Vec<Vec<RingMarkCell>> {
        (0..self.height / 2)
            .map(|row| {
                (0..self.width)
                    .map(|col| {
                        RingMarkCell::halves(
                            self.layer_at(col, row * 2),
                            self.layer_at(col, row * 2 + 1),
                        )
                    })
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
        }
    }

    fn halves(upper_layer: Option<RingMarkLayer>, lower_layer: Option<RingMarkLayer>) -> Self {
        let ch = match (upper_layer, lower_layer) {
            (None, None) => return Self::blank(),
            (Some(upper), Some(lower)) if upper == lower => '█',
            (Some(_), Some(_)) | (Some(_), None) => '▀',
            (None, Some(_)) => '▄',
        };

        Self {
            ch,
            layer: upper_layer.or(lower_layer),
            upper_layer,
            lower_layer,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Band {
    layer: RingMarkLayer,
    radius: f64,
    half_width: f64,
}

const BANDS: &[Band] = &[
    Band {
        layer: RingMarkLayer::Heartwood,
        radius: 0.12,
        half_width: 0.10,
    },
    Band {
        layer: RingMarkLayer::Inner,
        radius: 0.42,
        half_width: 0.06,
    },
    Band {
        layer: RingMarkLayer::Outer,
        radius: 0.67,
        half_width: 0.06,
    },
    Band {
        layer: RingMarkLayer::Cambium,
        radius: 0.925,
        half_width: 0.075,
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
struct PixelSample {
    col: usize,
    row: usize,
    center_x: f64,
    center_y: f64,
    radius_scale: f64,
    perspective: f64,
    shear: f64,
    highlight_angle: f64,
}

fn sample_pixel(
    sample: PixelSample,
    frame: usize,
    activity: RingMarkActivity,
) -> Option<RingMarkLayer> {
    let dx = sample.col as f64 - sample.center_x;
    let dy = sample.row as f64 - sample.center_y;
    let x = dx / sample.radius_scale;
    let y = (dy + dx * sample.shear) / (sample.radius_scale * sample.perspective);
    let radius = (x * x + y * y).sqrt();
    let angle = y.atan2(x);

    if radius > 1.10 {
        return None;
    }
    if scar_stroke(angle, radius, frame, activity) {
        return Some(RingMarkLayer::Scar);
    }
    if cut_gap(angle, radius) {
        return None;
    }
    band_for_radius(radius, angle, sample.highlight_angle, frame, activity)
}

fn band_for_radius(
    radius: f64,
    angle: f64,
    highlight_angle: f64,
    frame: usize,
    activity: RingMarkActivity,
) -> Option<RingMarkLayer> {
    BANDS
        .iter()
        .find(|band| {
            let band = **band;
            let center = animated_radius(band, frame, activity);
            let mut half_width = band.half_width + activity.layer(band.layer) * 0.030;
            if angular_distance(angle, highlight_angle) < 0.22 {
                half_width += 0.016;
            }
            (radius - center).abs() <= half_width
        })
        .map(|band| band.layer)
}

fn animated_radius(band: Band, frame: usize, activity: RingMarkActivity) -> f64 {
    let offset = pulse_index(band.layer) as f64 * 0.88;
    let wave = (frame as f64 * 0.34 + offset).sin();
    let ambient = 0.010 * wave;
    let live = activity.layer(band.layer) * (0.035 + 0.012 * wave.abs());
    (band.radius * (1.0 + ambient + live)).min(1.04)
}

fn scar_stroke(angle: f64, radius: f64, frame: usize, activity: RingMarkActivity) -> bool {
    let scar_activity = activity.layer(RingMarkLayer::Scar);
    let shimmer = 0.50 + ((frame as f64 * 0.42).sin() * 0.50);
    let width = 0.034 + scar_activity * 0.030 + shimmer * 0.010;
    let upper_edge = radius > 0.20 && radius < 1.05 && angular_distance(angle, -0.61) < width;
    let lower_edge = radius > 0.46 && radius < 0.94 && angular_distance(angle, 2.30) < width * 0.90;
    upper_edge || lower_edge
}

fn cut_gap(angle: f64, radius: f64) -> bool {
    if radius <= 0.20 {
        return false;
    }
    let upper_gap = radius > 0.22 && angular_distance(angle, -0.78) < 0.15 + radius * 0.03;
    let lower_gap = radius > 0.46 && angular_distance(angle, 2.35) < 0.085;
    upper_gap || lower_gap
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
                frame.pixels.iter().any(|pixel| *pixel == Some(layer)),
                "missing {layer:?}"
            );
        }
    }

    #[test]
    fn frame_is_backend_independent_layer_data() {
        let frame = RingMarkFrame::with_activity(31, 12, 0, RingMarkActivity::default());

        assert_eq!(frame.width, 31);
        assert_eq!(frame.height, 26);
        assert_eq!(frame.layer_at(usize::MAX, 0), None);
        assert_eq!(frame.layer_at(0, usize::MAX), None);
        assert_eq!(frame.half_block_rows().len(), 13);
    }

    #[test]
    fn mark_uses_terminal_raster_glyphs_instead_of_template_fill() {
        let rendered = ring_mark_rows_with_activity(31, 12, 0, RingMarkActivity::default())
            .into_iter()
            .flatten()
            .filter(|cell| cell.layer.is_some())
            .map(|cell| cell.ch)
            .collect::<String>();

        assert!(rendered.chars().all(is_raster_glyph));
        assert!(rendered.contains('▀'));
        assert!(rendered.contains('▄'));
        assert!(rendered.contains('█'));
        assert!(!rendered.contains('#'));
        assert!(!rendered.contains('='));
        assert!(!rendered.contains('o'));
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
            .filter(|pixel| **pixel == Some(RingMarkLayer::Cambium))
            .count();
        let live_cambium = live
            .pixels
            .iter()
            .filter(|pixel| **pixel == Some(RingMarkLayer::Cambium))
            .count();

        assert!(live_cambium > calm_cambium);
    }

    #[test]
    fn mark_keeps_requested_odd_dimensions() {
        let rows = ring_mark_rows_with_activity(23, 7, 1, RingMarkActivity::default());

        assert_eq!(rows.len(), 7);
        assert!(rows.iter().all(|row| row.len() == 23));
    }

    fn is_raster_glyph(ch: char) -> bool {
        matches!(ch, '█' | '▀' | '▄')
    }
}
