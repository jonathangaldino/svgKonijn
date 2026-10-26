use std::collections::HashMap;
use std::fmt::Write;

use image::{imageops, RgbaImage};
use rayon::prelude::*;
use serde::Deserialize;
use visioncortex::{BinaryImage, PathSimplifyMode, PointF64};

/// Keeps a slider at 200% on a huge photo from exhausting memory.
const MAX_PIXELS: u64 = 64_000_000;

#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Curves {
    Spline,
    Polygon,
    Pixel,
}

/// Mirrors the controls in the UI; percentages and degrees are kept in UI units.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Params {
    pub color: bool,
    pub posterize: bool,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
    pub scale: f32,
    pub rotation: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub grayscale: f32,
    pub hue_rotate: f32,
    pub invert: f32,
    pub opacity: f32,
    pub saturate: f32,
    pub sepia: f32,
    pub curves: Curves,
    pub speckle: usize,
    pub corner_threshold: f64,
    pub segment_length: f64,
    pub splice_threshold: f64,
    pub stroke_width: f32,
    pub min_path_segments: usize,
    pub precision: u32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            color: true,
            posterize: true,
            red: 7,
            green: 7,
            blue: 7,
            alpha: 1,
            scale: 100.0,
            rotation: 0.0,
            brightness: 100.0,
            contrast: 100.0,
            grayscale: 0.0,
            hue_rotate: 0.0,
            invert: 0.0,
            opacity: 100.0,
            saturate: 100.0,
            sepia: 0.0,
            curves: Curves::Spline,
            speckle: 2,
            corner_threshold: 60.0,
            segment_length: 4.0,
            splice_threshold: 45.0,
            stroke_width: 0.0,
            min_path_segments: 0,
            precision: 2,
        }
    }
}

pub fn trace(src: &RgbaImage, p: &Params) -> Result<String, String> {
    let img = preprocess(src, p)?;
    let (w, h) = img.dimensions();

    let mut layers: HashMap<[u8; 4], Layer> = HashMap::new();
    for (x, y, px) in img.enumerate_pixels() {
        let mut c = px.0;
        if c[3] == 0 {
            continue;
        }
        if !p.color {
            if c[3] < 128 || luma(c) >= 128.0 {
                continue;
            }
            c = [0, 0, 0, 255];
        }
        layers.entry(c).or_insert_with(Layer::default).add(x, y);
    }
    // Biggest layers first so small details paint on top of large areas.
    let mut layers: Vec<_> = layers.into_iter().collect();
    layers.sort_by(|a, b| b.1.pts.len().cmp(&a.1.pts.len()).then(a.0.cmp(&b.0)));

    let groups: Vec<String> = layers.par_iter().map(|(c, l)| trace_layer(*c, l, p)).collect();
    Ok(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">{}</svg>"#,
        groups.concat()
    ))
}

#[derive(Default)]
struct Layer {
    pts: Vec<(u32, u32)>,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl Layer {
    fn add(&mut self, x: u32, y: u32) {
        if self.pts.is_empty() {
            (self.x0, self.y0, self.x1, self.y1) = (x, y, x, y);
        }
        self.x0 = self.x0.min(x);
        self.y0 = self.y0.min(y);
        self.x1 = self.x1.max(x);
        self.y1 = self.y1.max(y);
        self.pts.push((x, y));
    }
}

fn trace_layer(c: [u8; 4], l: &Layer, p: &Params) -> String {
    let mut bin = BinaryImage::new_w_h((l.x1 - l.x0 + 1) as usize, (l.y1 - l.y0 + 1) as usize);
    for &(x, y) in &l.pts {
        bin.set_pixel((x - l.x0) as usize, (y - l.y0) as usize, true);
    }
    let mode = match p.curves {
        Curves::Spline => PathSimplifyMode::Spline,
        Curves::Polygon => PathSimplifyMode::Polygon,
        Curves::Pixel => PathSimplifyMode::None,
    };
    let mut paths = String::new();
    for cluster in bin.to_clusters(false) {
        if cluster.size() <= p.speckle {
            continue;
        }
        let path = cluster.to_compound_path(
            mode,
            p.corner_threshold.to_radians(),
            p.segment_length,
            10,
            p.splice_threshold.to_radians(),
        );
        let origin = PointF64::new(l.x0 as f64, l.y0 as f64);
        let (d, off) = path.to_svg_string(true, origin, Some(p.precision));
        if d.split_whitespace().count() < p.min_path_segments {
            continue;
        }
        let _ = write!(paths, r#"<path d="{}" transform="translate({},{})"/>"#, d.trim(), off.x, off.y);
    }
    if paths.is_empty() {
        return paths;
    }
    let hex = format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
    let mut attrs = format!(r#"fill="{hex}""#);
    let alpha = (c[3] < 255).then(|| format!("{:.2}", c[3] as f32 / 255.0));
    if let Some(a) = &alpha {
        let _ = write!(attrs, r#" fill-opacity="{a}""#);
    }
    if p.stroke_width > 0.0 {
        let _ = write!(attrs, r#" stroke="{hex}" stroke-width="{}""#, p.stroke_width);
        if let Some(a) = &alpha {
            let _ = write!(attrs, r#" stroke-opacity="{a}""#);
        }
    }
    format!("<g {attrs}>{paths}</g>")
}

fn preprocess(src: &RgbaImage, p: &Params) -> Result<RgbaImage, String> {
    let s = p.scale / 100.0;
    let (w, h) = (
        ((src.width() as f32 * s).round() as u32).max(1),
        ((src.height() as f32 * s).round() as u32).max(1),
    );
    if w as u64 * h as u64 > MAX_PIXELS {
        return Err(format!("{w}×{h} is too large to trace, lower the scale"));
    }
    let mut img = if (w, h) == src.dimensions() {
        src.clone()
    } else {
        imageops::resize(src, w, h, imageops::FilterType::Triangle)
    };
    if p.rotation.rem_euclid(360.0) != 0.0 {
        img = rotate(&img, p.rotation);
    }
    img.par_chunks_exact_mut(4).for_each(|px| {
        filter_pixel(px, p);
        if p.posterize {
            for (v, n) in px.iter_mut().zip([p.red, p.green, p.blue, p.alpha]) {
                *v = posterize(*v, n);
            }
        }
    });
    Ok(img)
}

/// Rotates clockwise around the center into an enlarged canvas so no corner gets clipped.
fn rotate(img: &RgbaImage, deg: f32) -> RgbaImage {
    let (s, c) = deg.to_radians().sin_cos();
    let (w, h) = (img.width() as f32, img.height() as f32);
    let nw = (w * c.abs() + h * s.abs()).round().max(1.0) as u32;
    let nh = (w * s.abs() + h * c.abs()).round().max(1.0) as u32;
    let (ncx, ncy) = (nw as f32 / 2.0, nh as f32 / 2.0);
    let mut out = RgbaImage::new(nw, nh);
    for (x, y, px) in out.enumerate_pixels_mut() {
        let (dx, dy) = (x as f32 + 0.5 - ncx, y as f32 + 0.5 - ncy);
        let (sx, sy) = (c * dx + s * dy + w / 2.0, -s * dx + c * dy + h / 2.0);
        if sx >= 0.0 && sy >= 0.0 && sx < w && sy < h {
            *px = *img.get_pixel(sx as u32, sy as u32);
        }
    }
    out
}

/// `n` steps give `n + 1` evenly spaced levels, so 1 step is a hard on/off.
fn posterize(v: u8, n: u8) -> u8 {
    let n = n.max(1) as f32;
    ((v as f32 / 255.0 * n).round() / n * 255.0).round() as u8
}

fn luma(c: [u8; 4]) -> f32 {
    0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32
}

/// CSS filter functions in the order the UI lists them (matrices from the Filter Effects spec).
fn filter_pixel(px: &mut [u8], p: &Params) {
    let mut c = [px[0], px[1], px[2]].map(|v| v as f32 / 255.0);
    let clamp = |c: [f32; 3]| c.map(|v| v.clamp(0.0, 1.0));
    if p.brightness != 100.0 {
        c = clamp(c.map(|v| v * p.brightness / 100.0));
    }
    if p.contrast != 100.0 {
        c = clamp(c.map(|v| (v - 0.5) * p.contrast / 100.0 + 0.5));
    }
    if p.grayscale != 0.0 {
        let s = 1.0 - p.grayscale / 100.0;
        c = clamp(mat(c, [
            [0.2126 + 0.7874 * s, 0.7152 - 0.7152 * s, 0.0722 - 0.0722 * s],
            [0.2126 - 0.2126 * s, 0.7152 + 0.2848 * s, 0.0722 - 0.0722 * s],
            [0.2126 - 0.2126 * s, 0.7152 - 0.7152 * s, 0.0722 + 0.9278 * s],
        ]));
    }
    if p.hue_rotate.rem_euclid(360.0) != 0.0 {
        let (s, k) = p.hue_rotate.to_radians().sin_cos();
        c = clamp(mat(c, [
            [0.213 + k * 0.787 - s * 0.213, 0.715 - k * 0.715 - s * 0.715, 0.072 - k * 0.072 + s * 0.928],
            [0.213 - k * 0.213 + s * 0.143, 0.715 + k * 0.285 + s * 0.140, 0.072 - k * 0.072 - s * 0.283],
            [0.213 - k * 0.213 - s * 0.787, 0.715 - k * 0.715 + s * 0.715, 0.072 + k * 0.928 + s * 0.072],
        ]));
    }
    if p.invert != 0.0 {
        let i = p.invert / 100.0;
        c = c.map(|v| i + v * (1.0 - 2.0 * i));
    }
    if p.saturate != 100.0 {
        let s = p.saturate / 100.0;
        c = clamp(mat(c, [
            [0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s],
            [0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s],
            [0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s],
        ]));
    }
    if p.sepia != 0.0 {
        let t = 1.0 - p.sepia / 100.0;
        c = clamp(mat(c, [
            [0.393 + 0.607 * t, 0.769 - 0.769 * t, 0.189 - 0.189 * t],
            [0.349 - 0.349 * t, 0.686 + 0.314 * t, 0.168 - 0.168 * t],
            [0.272 - 0.272 * t, 0.534 - 0.534 * t, 0.131 + 0.869 * t],
        ]));
    }
    for (o, v) in px.iter_mut().zip(c) {
        *o = (v * 255.0).round() as u8;
    }
    if p.opacity != 100.0 {
        px[3] = (px[3] as f32 * p.opacity / 100.0).round() as u8;
    }
}

fn mat(c: [f32; 3], m: [[f32; 3]; 3]) -> [f32; 3] {
    m.map(|r| r[0] * c[0] + r[1] * c[1] + r[2] * c[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn two_halves() -> RgbaImage {
        RgbaImage::from_fn(40, 20, |x, _| if x < 20 { Rgba([255, 0, 0, 255]) } else { Rgba([0, 0, 255, 255]) })
    }

    #[test]
    fn color_mode_emits_one_group_per_color() {
        let svg = trace(&two_halves(), &Params::default()).unwrap();
        assert!(svg.starts_with("<svg") && svg.contains(r#"viewBox="0 0 40 20""#));
        assert!(svg.contains(r##"fill="#ff0000""##) && svg.contains(r##"fill="#0000ff""##));
        assert_eq!(svg.matches("<g ").count(), 2);
    }

    #[test]
    fn monochrome_traces_only_dark_pixels() {
        let p = Params { color: false, ..Default::default() };
        let blank = RgbaImage::from_pixel(10, 10, Rgba([255, 255, 255, 255]));
        assert!(!trace(&blank, &p).unwrap().contains("<path"));
        let mut img = blank.clone();
        for y in 2..8 {
            for x in 2..8 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        assert_eq!(trace(&img, &p).unwrap().matches("<path").count(), 1);
    }

    #[test]
    fn speckles_are_dropped() {
        let mut img = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        img.put_pixel(5, 5, Rgba([0, 0, 0, 255]));
        assert!(!trace(&img, &Params::default()).unwrap().contains("<path"));
        let keep = Params { speckle: 0, ..Default::default() };
        assert!(trace(&img, &keep).unwrap().contains("<path"));
    }

    #[test]
    fn rotate_quarter_turn_swaps_dimensions() {
        let r = rotate(&two_halves(), 90.0);
        assert_eq!(r.dimensions(), (20, 40));
        // Clockwise: the red left half ends up on top.
        assert_eq!(r.get_pixel(10, 5).0, [255, 0, 0, 255]);
    }

    #[test]
    fn posterize_levels() {
        assert_eq!([0, 100, 200, 255].map(|v| posterize(v, 1)), [0, 0, 255, 255]);
        assert_eq!(posterize(128, 2), 128);
    }

    #[test]
    fn rejects_oversized_scale() {
        let p = Params { scale: 200.0, ..Default::default() };
        assert!(trace(&RgbaImage::new(5000, 5000), &p).is_err());
    }
}
