//! Icon generation, replacing the retired `scripts/gen-icons.py` (Pillow)
//! with a pure-Rust equivalent that reuses the same artwork parameters.
//!
//! Run: `cargo run --example gen-icons`

use std::fs;
use std::io::{Cursor, Seek, Write};
use std::path::{Path, PathBuf};

use image::{ImageBuffer, ImageFormat, Rgba, RgbaImage, imageops::FilterType};

const ICONS_DIR: &str = "icons";
const SS: i32 = 4; // supersample factor so downscaled edges stay smooth

const PURPLE: Rgba<u8> = Rgba([108, 62, 190, 255]);
const PURPLE_DARK: Rgba<u8> = Rgba([74, 39, 140, 255]);
const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
const TRANSPARENT: Rgba<u8> = Rgba([0, 0, 0, 0]);

// ── drawing primitives ───────────────────────────────────────────────────

/// Fills `x0..=x1` by `y0..=y1`, clamped to the canvas.
fn fill_rect(img: &mut RgbaImage, (x0, y0): (i32, i32), (x1, y1): (i32, i32), color: Rgba<u8>) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    for y in y0.max(0)..=y1.min(h - 1) {
        for x in x0.max(0)..=x1.min(w - 1) {
            img.put_pixel(x as u32, y as u32, color);
        }
    }
}

/// True when the point lies inside the box with four rounded corners.
fn in_rounded_rect(origin: (i32, i32), corner: (i32, i32), radius: i32, x: i32, y: i32) -> bool {
    if x < origin.0 || x > corner.0 || y < origin.1 || y > corner.1 {
        return false;
    }
    let ax = x.clamp(origin.0 + radius, corner.0 - radius);
    let ay = y.clamp(origin.1 + radius, corner.1 - radius);
    let (dx, dy) = (i64::from(x - ax), i64::from(y - ay));
    dx * dx + dy * dy <= i64::from(radius) * i64::from(radius)
}

fn fill_rounded_rect(
    img: &mut RgbaImage,
    origin: (i32, i32),
    corner: (i32, i32),
    radius: i32,
    color: Rgba<u8>,
) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    for y in origin.1.max(0)..=corner.1.min(h - 1) {
        for x in origin.0.max(0)..=corner.0.min(w - 1) {
            if in_rounded_rect(origin, corner, radius, x, y) {
                img.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

/// Outline of a rounded rect drawn with `width`-thick stroke.
fn stroke_rounded_rect(
    img: &mut RgbaImage,
    origin: (i32, i32),
    corner: (i32, i32),
    radius: i32,
    width: i32,
    color: Rgba<u8>,
) {
    let inner_origin = (origin.0 + width, origin.1 + width);
    let inner_corner = (corner.0 - width, corner.1 - width);
    let inner_radius = (radius - width).max(0);
    let (canv_w, canv_h) = (img.width() as i32, img.height() as i32);
    for y in origin.1.max(0)..=corner.1.min(canv_h - 1) {
        for x in origin.0.max(0)..=corner.0.min(canv_w - 1) {
            let outer = in_rounded_rect(origin, corner, radius, x, y);
            let inner = in_rounded_rect(inner_origin, inner_corner, inner_radius, x, y);
            if outer && !inner {
                img.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

/// Even-odd scanline polygon fill, so the fold corner can be cut away.
fn fill_polygon(img: &mut RgbaImage, points: &[(i32, i32)], color: Rgba<u8>) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let ymin = points.iter().map(|p| p.1).min().unwrap_or(0).max(0);
    let ymax = points.iter().map(|p| p.1).max().unwrap_or(0).min(h - 1);

    for y in ymin..=ymax {
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..points.len() {
            let (xa, ya) = points[i];
            let (xb, yb) = points[(i + 1) % points.len()];
            if (ya <= y && yb > y) || (yb <= y && ya > y) {
                let t = f64::from(y - ya) / f64::from(yb - ya);
                xs.push(f64::from(xa) + t * f64::from(xb - xa));
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        for pair in xs.chunks(2) {
            if pair.len() == 2 {
                let x_from = (pair[0] as i32).max(0);
                let x_to = ((pair[1] as i32) - 1).min(w - 1);
                fill_rect(img, (x_from, y), (x_to, y), color);
            }
        }
    }
}

// ── artwork ──────────────────────────────────────────────────────────────

/// Notebook page with folded corner, spine, and ruled lines.
fn rounded_page(size: u32) -> RgbaImage {
    let s = i32::try_from(size).unwrap_or(0) * SS;
    let m: i32 = (f64::from(s) * 0.12) as i32;
    let fold: i32 = (f64::from(s) * 0.22) as i32;
    let radius: i32 = (f64::from(s) * 0.08) as i32;

    let mut img = ImageBuffer::from_pixel(
        u32::try_from(s).unwrap_or(0),
        u32::try_from(s).unwrap_or(0),
        TRANSPARENT,
    );

    // Body
    fill_rounded_rect(&mut img, (m, m), (s - m, s - m), radius, PURPLE);

    // Folded top-right corner: the square half is cut away...
    fill_polygon(&mut img, &[(s - m - fold, m), (s - m, m), (s - m, m + fold)], TRANSPARENT);
    // ...and replaced by a darker triangle.
    fill_polygon(
        &mut img,
        &[(s - m - fold, m), (s - m, m + fold), (s - m - fold, m + fold)],
        PURPLE_DARK,
    );

    // Spine down the left edge
    fill_rounded_rect(
        &mut img,
        (m, m),
        (m + (f64::from(s) * 0.055) as i32, s - m),
        (f64::from(s) * 0.02) as i32,
        PURPLE_DARK,
    );

    // Ruled lines (the last one is short)
    let line_x0 = m + (f64::from(s) * 0.15) as i32;
    let line_x1 = s - m - (f64::from(s) * 0.12) as i32;
    let thickness = ((f64::from(s) * 0.035) as i32).max(1);
    for (i, frac) in [0.40, 0.52, 0.64, 0.76].iter().enumerate() {
        let y = (f64::from(s) * frac) as i32;
        let mut length = line_x1 - line_x0;
        if i == 3 {
            length = (f64::from(length) * 0.6) as i32;
        }
        fill_rect(&mut img, (line_x0, y), (line_x0 + length, y + thickness), WHITE);
    }

    image::imageops::resize(&img, size, size, FilterType::Lanczos3)
}

/// Monochrome-friendly glyph for the system tray.
fn tray_icon(size: u32) -> RgbaImage {
    let s = i32::try_from(size).unwrap_or(0) * SS;
    let stroke = ((f64::from(s) * 0.09) as i32).max(2);
    let radius = (f64::from(s) * 0.18) as i32;

    let mut img = ImageBuffer::from_pixel(
        u32::try_from(s).unwrap_or(0),
        u32::try_from(s).unwrap_or(0),
        TRANSPARENT,
    );

    stroke_rounded_rect(
        &mut img,
        (stroke, stroke),
        (s - stroke, s - stroke),
        radius,
        stroke,
        WHITE,
    );

    let inset = stroke + (f64::from(s) * 0.22) as i32;
    for frac in [0.42, 0.58] {
        let y = (f64::from(s) * frac) as i32;
        fill_rect(&mut img, (inset, y), (s - inset, y + stroke), WHITE);
    }

    image::imageops::resize(&img, size, size, FilterType::Lanczos3)
}

/// Same SVG as the retired Python version, so re-runs are byte-identical.
const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" width="128" height="128">
  <defs>
    <clipPath id="page">
      <path d="M16 8h72l24 24v88a4 4 0 0 1-4 4H16a4 4 0 0 1-4-4V12a4 4 0 0 1 4-4z"/>
    </clipPath>
  </defs>
  <path d="M16 8h72l24 24v88a4 4 0 0 1-4 4H16a4 4 0 0 1-4-4V12a4 4 0 0 1 4-4z"
        fill="#6c3ebe"/>
  <g clip-path="url(#page)">
    <rect x="12" y="8" width="12" height="112" fill="#4a278c"/>
    <path d="M88 8h24v24z" fill="#4a278c"/>
  </g>
  <g fill="#ffffff">
    <rect x="38" y="52" width="62" height="7" rx="3.5"/>
    <rect x="38" y="68" width="62" height="7" rx="3.5"/>
    <rect x="38" y="84" width="62" height="7" rx="3.5"/>
    <rect x="38" y="100" width="38" height="7" rx="3.5"/>
  </g>
</svg>
"##;

// ── multi-resolution .ico ────────────────────────────────────────────────

/// ICONDIR plus one ICONDIRENTRY per frame. Width and height are stored as
/// single bytes where 0 means 256 (the maximum the format can express).
fn write_ico(path: &Path, sizes: &[u32]) -> Result<(), Box<dyn std::error::Error>> {
    let frames: Vec<Vec<u8>> = sizes.iter().map(|&size| png_bytes(rounded_page(size))).collect();

    let mut out: Vec<u8> = Vec::new();
    let count = u16::try_from(frames.len())?;
    out.extend(0u16.to_le_bytes()); // reserved, must be 0
    out.extend(1u16.to_le_bytes()); // type: 1 = icon
    out.extend(count.to_le_bytes());

    let mut offset = 6 + (u32::from(count) * 16);
    for (frame, &size) in frames.iter().zip(sizes) {
        let entry_side = if size >= 256 { 0 } else { u8::try_from(size)? };
        out.extend([entry_side, entry_side]); // width, height
        out.extend([0u8, 0u8]); // colours in palette, reserved
        out.extend(1u16.to_le_bytes()); // colour planes
        out.extend(32u16.to_le_bytes()); // bits per pixel
        out.extend(u32::try_from(frame.len())?.to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += u32::try_from(frame.len())?;
    }
    for frame in frames {
        out.extend(frame);
    }

    fs::write(path, out)?;
    Ok(())
}

fn png_bytes(img: RgbaImage) -> Vec<u8> {
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .expect("PNG is infallible for RGBA8");
    buf
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(ICONS_DIR);
    fs::create_dir_all(&out)?;

    for (name, size) in [
        ("32x32.png", 32),
        ("128x128.png", 128),
        ("128x128@2x.png", 256),
        ("icon.png", 512),
    ] {
        let path = out.join(name);
        rounded_page(size).save(&path)?;
        println!("wrote icons/{name} ({size}x{size})");
    }

    let tray = out.join("tray.png");
    tray_icon(22).save(&tray)?;
    println!("wrote icons/tray.png (22x22)");

    let svg = out.join("icon.svg");
    fs::write(&svg, SVG)?;
    println!("wrote icons/icon.svg (scalable)");

    // Multi-resolution .ico for Windows targets of the same source tree.
    let ico = out.join("icon.ico");
    write_ico(&ico, &[16, 32, 48, 64, 128, 256])?;
    println!("wrote icons/icon.ico");

    Ok(())
}
