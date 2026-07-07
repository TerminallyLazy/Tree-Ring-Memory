use std::f64::consts::PI;
use std::sync::OnceLock;

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
        let pixels = sample_tree_ring_object(width, height, frame, activity);

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

    fn shaded(ch: char, layer: RingMarkLayer, brightness: u8) -> Self {
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
}

const BANDS: &[Band] = &[
    Band {
        layer: RingMarkLayer::Heartwood,
        radius: 0.16,
        half_width: 0.052,
    },
    Band {
        layer: RingMarkLayer::Inner,
        radius: 0.34,
        half_width: 0.044,
    },
    Band {
        layer: RingMarkLayer::Inner,
        radius: 0.49,
        half_width: 0.036,
    },
    Band {
        layer: RingMarkLayer::Outer,
        radius: 0.64,
        half_width: 0.038,
    },
    Band {
        layer: RingMarkLayer::Outer,
        radius: 0.78,
        half_width: 0.035,
    },
    Band {
        layer: RingMarkLayer::Cambium,
        radius: 0.94,
        half_width: 0.046,
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
struct PlotPoint {
    x: f64,
    y: f64,
    z: f64,
    nx: f64,
    ny: f64,
    nz: f64,
    layer: RingMarkLayer,
    activity: f64,
}

fn sample_tree_ring_object(
    width: usize,
    height: usize,
    frame: usize,
    activity: RingMarkActivity,
) -> Vec<RingMarkCell> {
    let sub_width = width * 3;
    let sub_height = height * 3;
    let mut hits = vec![None; sub_width * sub_height];
    let mut z_buffer = vec![f64::NEG_INFINITY; sub_width * sub_height];
    let phase = frame as f64 * 0.085;

    sample_ring_tubes(
        sub_width,
        sub_height,
        phase,
        activity,
        &mut hits,
        &mut z_buffer,
    );
    sample_scar(
        sub_width,
        sub_height,
        phase,
        activity.layer(RingMarkLayer::Scar),
        &mut hits,
        &mut z_buffer,
    );

    let mut cells = Vec::with_capacity(width * height);
    for row in 0..height {
        for col in 0..width {
            let mut pattern = 0u16;
            let mut brightness_sum = 0usize;
            let mut layer_counts = [0usize; 5];
            for sub_y in 0..3 {
                for sub_x in 0..3 {
                    let sx = col * 3 + sub_x;
                    let sy = row * 3 + sub_y;
                    let bit = (sub_y * 3 + sub_x) as u16;
                    if let Some(hit) = hits[sy * sub_width + sx] {
                        pattern |= 1 << bit;
                        brightness_sum += hit.brightness as usize;
                        layer_counts[pulse_index(hit.layer)] += 1;
                    }
                }
            }
            if pattern == 0 {
                cells.push(RingMarkCell::blank());
            } else {
                let layer = dominant_layer(layer_counts);
                let brightness = (brightness_sum / pattern.count_ones() as usize) as u8;
                cells.push(RingMarkCell::shaded(
                    shape_char_3x3(pattern, brightness),
                    layer,
                    brightness,
                ));
            }
        }
    }
    cells
}

fn sample_ring_tubes(
    sub_width: usize,
    sub_height: usize,
    phase: f64,
    activity: RingMarkActivity,
    hits: &mut [Option<SubPixelHit>],
    z_buffer: &mut [f64],
) {
    static THETA: OnceLock<Vec<(f64, f64)>> = OnceLock::new();
    let theta_table = THETA.get_or_init(|| angle_table(0.070));

    for band in BANDS {
        let band_activity = activity.layer(band.layer);
        let radius = animated_radius(*band, phase, band_activity);
        let tube_radius = band.half_width + band_activity * 0.026;
        let offset_scales: &[f64] = if band_activity > 0.45 {
            &[-0.90, 0.0, 0.90]
        } else {
            &[0.0]
        };
        for &(cos_theta, sin_theta) in theta_table {
            for &offset_scale in offset_scales {
                let ring_radius = radius + tube_radius * offset_scale;
                let point = PlotPoint {
                    x: ring_radius * cos_theta,
                    y: ring_radius * sin_theta,
                    z: 0.020 * (pulse_index(band.layer) as f64 - 2.0),
                    nx: 0.0,
                    ny: 0.0,
                    nz: 1.0,
                    layer: band.layer,
                    activity: band_activity,
                };
                plot_point(point, sub_width, sub_height, phase, hits, z_buffer);
            }
        }
    }

    let side_depth = [-0.28, -0.15, -0.02, 0.11];
    for &(cos_theta, sin_theta) in theta_table {
        let bark_wave = (phase * 2.0 + cos_theta * 3.0 + sin_theta * 5.0).sin() * 0.018;
        let radius = 1.02 + bark_wave;
        for z in side_depth {
            let point = PlotPoint {
                x: radius * cos_theta,
                y: radius * sin_theta,
                z,
                nx: cos_theta,
                ny: sin_theta,
                nz: 0.16,
                layer: RingMarkLayer::Cambium,
                activity: activity.layer(RingMarkLayer::Cambium),
            };
            plot_point(point, sub_width, sub_height, phase, hits, z_buffer);
        }
    }
}

fn animated_radius(band: Band, phase: f64, layer_activity: f64) -> f64 {
    let offset = pulse_index(band.layer) as f64 * 0.88;
    let wave = (phase * 4.0 + offset).sin();
    let ambient = 0.010 * wave;
    let live = layer_activity * (0.035 + 0.012 * wave.abs());
    (band.radius * (1.0 + ambient + live)).min(1.04)
}

fn sample_scar(
    sub_width: usize,
    sub_height: usize,
    phase: f64,
    scar_activity: f64,
    hits: &mut [Option<SubPixelHit>],
    z_buffer: &mut [f64],
) {
    let width = 0.026 + scar_activity * 0.034;
    for index in 0..78 {
        let t = -0.78 + index as f64 * 0.020;
        let center_x = 0.18 + t * 0.42 + (t * 6.0 + phase).sin() * 0.030;
        let center_y = -0.52 * t + (t * 3.2).sin() * 0.075;
        for side in -2..=2 {
            let offset = side as f64 * width;
            let point = PlotPoint {
                x: center_x + offset,
                y: center_y - offset * 0.35,
                z: 0.080 + (t * 5.0 + phase * 2.0).sin() * 0.025,
                nx: -0.20,
                ny: 0.15,
                nz: 0.95,
                layer: RingMarkLayer::Scar,
                activity: scar_activity,
            };
            plot_point(point, sub_width, sub_height, phase, hits, z_buffer);
        }
    }
}

fn plot_point(
    point: PlotPoint,
    sub_width: usize,
    sub_height: usize,
    phase: f64,
    hits: &mut [Option<SubPixelHit>],
    z_buffer: &mut [f64],
) {
    let rot_x = -0.82 + (phase * 0.55).sin() * 0.16;
    let rot_y = phase * 1.25;
    let rot_z = (phase * 0.35).sin() * 0.24;
    let (x, y, z) = rotate_xyz(point.x, point.y, point.z, rot_x, rot_y, rot_z);
    let (nx, ny, nz) = rotate_xyz(point.nx, point.ny, point.nz, rot_x, rot_y, rot_z);
    let depth = 2.65 + z;
    if depth <= 0.25 {
        return;
    }

    let perspective = 1.0 / depth;
    let center_x = (sub_width as f64 - 1.0) * 0.5;
    let center_y = (sub_height as f64 - 1.0) * 0.52;
    let screen_x = center_x + x * sub_width as f64 * 1.02 * perspective;
    let screen_y = center_y - y * sub_height as f64 * 1.44 * perspective;
    let sx = screen_x.round() as isize;
    let sy = screen_y.round() as isize;
    if sx < 0 || sy < 0 || sx >= sub_width as isize || sy >= sub_height as isize {
        return;
    }

    let index = sy as usize * sub_width + sx as usize;
    if z <= z_buffer[index] {
        return;
    }

    z_buffer[index] = z;
    hits[index] = Some(SubPixelHit {
        layer: point.layer,
        brightness: shade(nx, ny, nz, z, point.activity),
    });
}

fn shade(nx: f64, ny: f64, nz: f64, z: f64, activity: f64) -> u8 {
    let light = normalize3(-0.35, 0.72, 1.00);
    let normal = normalize3(nx, ny, nz);
    let diffuse = dot(normal, light).max(0.0);
    let rim = (z + 0.95).clamp(0.0, 1.9) / 1.9;
    let glow = activity * 0.20;
    ((0.22 + diffuse * 0.54 + rim * 0.18 + glow).clamp(0.0, 1.0) * 255.0).round() as u8
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

fn shape_char_3x3(pattern: u16, brightness: u8) -> char {
    let count = pattern.count_ones();
    if count == 0 {
        return ' ';
    }

    let top_l = pattern & (1 << 0) != 0;
    let top_c = pattern & (1 << 1) != 0;
    let top_r = pattern & (1 << 2) != 0;
    let mid_l = pattern & (1 << 3) != 0;
    let mid_c = pattern & (1 << 4) != 0;
    let mid_r = pattern & (1 << 5) != 0;
    let bot_l = pattern & (1 << 6) != 0;
    let bot_c = pattern & (1 << 7) != 0;
    let bot_r = pattern & (1 << 8) != 0;
    let top = top_l as u8 + top_c as u8 + top_r as u8;
    let mid = mid_l as u8 + mid_c as u8 + mid_r as u8;
    let bot = bot_l as u8 + bot_c as u8 + bot_r as u8;
    let left = top_l as u8 + mid_l as u8 + bot_l as u8;
    let center = top_c as u8 + mid_c as u8 + bot_c as u8;
    let right = top_r as u8 + mid_r as u8 + bot_r as u8;

    let bright = brightness > 168;
    let midtone = brightness > 92;

    if count >= 8 {
        return if bright { '@' } else { '#' };
    }
    if top_l && mid_c && bot_r && !top_r && !bot_l {
        return if midtone { '\\' } else { '.' };
    }
    if top_r && mid_c && bot_l && !top_l && !bot_r {
        return if midtone { '/' } else { '.' };
    }
    if mid >= 2 && mid >= top && mid >= bot {
        return if bright { '=' } else { '-' };
    }
    if center >= 2 && center >= left && center >= right {
        return if midtone { '|' } else { ':' };
    }
    if left >= 2 && right == 0 {
        return if midtone { '(' } else { ':' };
    }
    if right >= 2 && left == 0 {
        return if midtone { ')' } else { ':' };
    }
    if top >= 2 && bot == 0 {
        return if bright { '"' } else { '\'' };
    }
    if bot >= 2 && top == 0 {
        return if bright { '_' } else { '.' };
    }
    if count >= 6 {
        return if bright { '%' } else { '*' };
    }
    if count >= 4 {
        return if midtone { '+' } else { ':' };
    }
    if count == 1 && (top_l || top_c || top_r) {
        return '\'';
    }
    if count <= 2 {
        return if midtone { ':' } else { '.' };
    }
    if mid_c {
        return if bright { 'o' } else { '*' };
    }
    if bright {
        '+'
    } else {
        ':'
    }
}

fn angle_table(step: f64) -> Vec<(f64, f64)> {
    let mut table = Vec::new();
    let mut angle = 0.0;
    while angle < PI * 2.0 {
        table.push((angle.cos(), angle.sin()));
        angle += step;
    }
    table
}

fn rotate_xyz(x: f64, y: f64, z: f64, ax: f64, ay: f64, az: f64) -> (f64, f64, f64) {
    let (cx, sx) = (ax.cos(), ax.sin());
    let (cy, sy) = (ay.cos(), ay.sin());
    let (cz, sz) = (az.cos(), az.sin());
    let (y1, z1) = (y * cx - z * sx, y * sx + z * cx);
    let (x2, z2) = (x * cy + z1 * sy, -x * sy + z1 * cy);
    let (x3, y3) = (x2 * cz - y1 * sz, x2 * sz + y1 * cz);
    (x3, y3, z2)
}

fn normalize3(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let length = (x * x + y * y + z * z).sqrt().max(f64::EPSILON);
    (x / length, y / length, z / length)
}

fn dot(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    a.0 * b.0 + a.1 * b.1 + a.2 * b.2
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
    fn mark_uses_3d_shading_glyphs_instead_of_template_fill() {
        let rendered = ring_mark_rows_with_activity(31, 12, 0, RingMarkActivity::default())
            .into_iter()
            .flatten()
            .filter(|cell| cell.layer.is_some())
            .map(|cell| cell.ch)
            .collect::<String>();

        assert!(rendered.chars().all(is_3d_glyph));
        assert!(
            rendered.contains('┃')
                || rendered.contains('━')
                || rendered.contains('|')
                || rendered.contains('-')
                || rendered.contains('/')
                || rendered.contains('\\')
        );
        assert!(!rendered.contains('█'));
        assert!(!rendered.contains('▀'));
        assert!(!rendered.contains('▄'));
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

    fn is_3d_glyph(ch: char) -> bool {
        matches!(
            ch,
            '.' | ':'
                | '*'
                | '+'
                | 'o'
                | '@'
                | '#'
                | '%'
                | '='
                | '-'
                | '|'
                | '/'
                | '\\'
                | '('
                | ')'
                | '"'
                | '\''
                | '_'
        )
    }
}
