use crate::color::Color;
use crate::font;
use crate::{info, warning};
use std::collections::HashMap;

#[inline(always)]
fn blend_channel(foreground: u8, background: u8, alpha: u8) -> u8 {
    let fg = foreground as i32;
    let bg = background as i32;
    let alpha = alpha as i32;
    let value = bg * 255 + (fg - bg) * alpha;

    // Division by 255 for 16-bit integers.
    ((value * 257 + 257) >> 16) as u8
}

#[inline(always)]
fn blend_pixel(foreground: Color, background: Color, alpha: u8) -> [u8; 4] {
    [
        blend_channel(foreground.b, background.b, alpha),
        blend_channel(foreground.g, background.g, alpha),
        blend_channel(foreground.r, background.r, alpha),
        255,
    ]
}

fn blend_pixels(pixels: &mut [u8], bitmap: &[u8], foreground: Color, background: Color) {
    let (pixels, _) = pixels.as_chunks_mut::<4>();
    for (pixel, &alpha) in pixels.iter_mut().zip(bitmap) {
        *pixel = blend_pixel(foreground, background, alpha);
    }
}

#[inline(always)]
fn blend_border(fg: u8, bg: u8, br: u8, inner: f32, outer: f32) -> u8 {
    (fg as f32 * inner + br as f32 * (outer - inner) + bg as f32 * (1.0 - outer)).round() as u8
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct CacheKey {
    pub c: char,
    pub ft_size: u32,
    pub ft_color: Color,
    pub bg_color: Color,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct CornerKey {
    radius: u32,
    fg_color: Color,
    bg_color: Color,
    br_color: Color,
    br_top: u32,
    br_left: u32,
}

// TODO: Extract (width, height, pixels) into a separate struct.
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub xmin: i32,
    pub ymin: i32,
    pub advance_width: f32,
    pub pixels: Vec<u8>, // BGRA
}

fn render_corner(key: CornerKey) -> Bitmap {
    let CornerKey {
        radius,
        fg_color,
        bg_color,
        br_color,
        br_top,
        br_left,
    } = key;
    let n = radius as usize;
    let r = radius as f32;
    let mut pixels = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        let dy = y as f32 + 0.5 - r;
        let dy_sq = dy * dy;

        for x in 0..n {
            let dx = x as f32 + 0.5 - r;
            let dx_sq = dx * dx;
            let ds_sq = dx_sq + dy_sq;

            let ds_edge = ds_sq.sqrt() - r;
            let outer = 0.5 - ds_edge;
            if outer <= 0.0 {
                pixels.extend_from_slice(&[bg_color.b, bg_color.g, bg_color.r, 255]);
                continue;
            }

            let br = if br_top == br_left {
                br_top as f32
            } else {
                br_top as f32 + (br_left as f32 - br_top as f32) * dx_sq / ds_sq
            };
            let inner = outer - br;
            if inner >= 1.0 {
                pixels.extend_from_slice(&[fg_color.b, fg_color.g, fg_color.r, 255]);
                continue;
            }
            if outer >= 1.0 && inner <= 0.0 {
                pixels.extend_from_slice(&[br_color.b, br_color.g, br_color.r, 255]);
                continue;
            }

            let outer = outer.clamp(0.0, 1.0);
            let inner = inner.clamp(0.0, 1.0);
            pixels.extend_from_slice(&[
                blend_border(fg_color.b, bg_color.b, br_color.b, inner, outer),
                blend_border(fg_color.g, bg_color.g, br_color.g, inner, outer),
                blend_border(fg_color.r, bg_color.r, br_color.r, inner, outer),
                255,
            ]);
        }
    }
    Bitmap {
        width: n,
        height: n,
        xmin: 0,
        ymin: 0,
        advance_width: 0.0,
        pixels,
    }
}

pub struct Rasterizer {
    fonts: Vec<font::Definition>,
    cache: HashMap<CacheKey, Bitmap>,
    corners: HashMap<CornerKey, Bitmap>,
}

impl Rasterizer {
    pub fn new(fonts: Vec<font::Definition>) -> Self {
        Self {
            fonts,
            cache: HashMap::new(),
            corners: HashMap::new(),
        }
    }

    pub fn ascent(&self, ft_size: u32) -> i32 {
        let Some(metrics) = self.fonts[0].font.horizontal_line_metrics(ft_size as f32) else {
            info!(
                "No horizontal line metrics for font {:?}",
                self.fonts[0].font.name()
            );
            return ft_size as i32;
        };

        metrics.ascent as i32
    }

    pub fn rasterize(
        &mut self,
        c: char,
        ft_size: u32,
        ft_color: Color,
        bg_color: Color,
    ) -> &Bitmap {
        let key = CacheKey {
            c,
            ft_size,
            ft_color,
            bg_color,
        };
        self.cache.entry(key).or_insert_with(|| {
            let (metrics, bitmap) = {
                let definition = self
                    .fonts
                    .iter()
                    .find(|d| d.font.lookup_glyph_index(c) > 0)
                    .unwrap_or_else(|| {
                        warning!("No configured font can render {}", c);
                        &self.fonts[0]
                    });
                definition.font.rasterize(c, ft_size as f32)
            };

            let mut pixels = vec![0u8; metrics.width * metrics.height * 4];
            blend_pixels(&mut pixels, &bitmap, ft_color, bg_color);
            Bitmap {
                width: metrics.width,
                height: metrics.height,
                xmin: metrics.xmin,
                ymin: metrics.ymin,
                advance_width: metrics.advance_width,
                pixels,
            }
        })
    }

    /// Rasterizes a top-left corner with [top, left] border widths.
    pub fn corner(
        &mut self,
        radius: u32,
        fg_color: Color,
        bg_color: Color,
        br_color: Color,
        br_top: u32,
        br_left: u32,
    ) -> &Bitmap {
        let key = CornerKey {
            radius,
            fg_color,
            bg_color,
            br_color,
            br_top,
            br_left,
        };
        self.corners
            .entry(key)
            .or_insert_with(|| render_corner(key))
    }

    pub fn get_default_font_size(&self, scale: i32) -> u32 {
        self.fonts[0].size * scale as u32
    }

    pub fn get_font_size(&self, text: &str, scale: i32) -> u32 {
        let (mut max_size, mut len) = (0, 0);
        for c in text.chars() {
            let s = self
                .fonts
                .iter()
                .find(|d| d.font.lookup_glyph_index(c) > 0)
                .map(|d| d.size)
                .unwrap_or(self.fonts[0].size);
            max_size = max_size.max(s);
            len += 1;
        }

        if len > 2 {
            max_size * scale as u32 * 2 / len
        } else {
            max_size * scale as u32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::util::bench;
    use std::hint::black_box;

    #[test]
    fn corner_cache() {
        let mut r = Rasterizer::new(Vec::new());
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        let key = CornerKey {
            radius: 4,
            fg_color: white,
            bg_color: black,
            br_color: black,
            br_top: 0,
            br_left: 0,
        };
        for corner in [
            // New entries
            key,
            CornerKey { radius: 2, ..key },
            CornerKey {
                fg_color: black,
                ..key
            },
            CornerKey {
                bg_color: white,
                ..key
            },
            CornerKey { br_top: 1, ..key },
            CornerKey { br_left: 1, ..key },
            CornerKey {
                br_color: white,
                ..key
            },
            // Re-used entries
            key,
            CornerKey {
                br_color: white,
                ..key
            },
        ] {
            r.corner(
                corner.radius,
                corner.fg_color,
                corner.bg_color,
                corner.br_color,
                corner.br_top,
                corner.br_left,
            );
        }
        assert_eq!(r.corners.len(), 7);
    }

    #[test]
    #[ignore = "run in release mode with --ignored --nocapture"]
    fn bench_render_corner() {
        for radius in [8, 16, 32] {
            for (case, br_top, br_left) in [
                ("no_border", 0, 0),
                ("equal_borders", 2, 2),
                ("unequal_borders", 2, 4),
            ] {
                let key = CornerKey {
                    radius,
                    fg_color: Color::rgb(37, 149, 213),
                    bg_color: Color::rgb(219, 83, 11),
                    br_color: Color::rgb(61, 173, 97),
                    br_top,
                    br_left,
                };
                let name = format!("render_corner/radius={radius}/{case}");

                bench(&name, || {
                    black_box(render_corner(black_box(key)));
                });
            }
        }
    }

    #[test]
    #[ignore = "run in release mode with --ignored --nocapture"]
    fn bench_methods() {
        let foreground = Color::rgb(37, 149, 213);
        let background = Color::rgb(219, 83, 11);
        let bitmap: Vec<u8> = (0..32 * 32).map(|i| i as u8).collect();
        let mut pixels = vec![0; bitmap.len() * 4];
        bench("blend_pixels", || {
            blend_pixels(
                black_box(&mut pixels),
                black_box(&bitmap),
                black_box(foreground),
                black_box(background),
            );
            black_box(&pixels);
        });
    }
}
