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

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct CacheKey {
    pub c: char,
    pub ft_size: u32,
    pub ft_color: Color,
    pub bg_color: Color,
}

pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub xmin: i32,
    pub ymin: i32,
    pub advance_width: f32,
    pub pixels: Vec<u8>, // BGRA
}

pub struct Rasterizer {
    fonts: Vec<font::Definition>,
    cache: HashMap<CacheKey, Bitmap>,
}

impl Rasterizer {
    pub fn new(fonts: Vec<font::Definition>) -> Self {
        Self {
            fonts,
            cache: HashMap::new(),
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
