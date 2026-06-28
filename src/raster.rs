use crate::color::Color;
use crate::font;
use crate::{info, warning};
use std::collections::HashMap;

macro_rules! blend {
    ($ft_color:expr, $bg_color:expr, $alpha:expr) => {
        ($ft_color as u32 * $alpha + $bg_color as u32 * (255 - $alpha)) / 255
    };
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
            let (chunks, _) = pixels.as_chunks_mut::<4>();
            for (chunk, alpha) in chunks.iter_mut().zip(bitmap) {
                let alpha = alpha as u32;
                let b = blend!(ft_color.b, bg_color.b, alpha);
                let g = blend!(ft_color.g, bg_color.g, alpha);
                let r = blend!(ft_color.r, bg_color.r, alpha);
                *chunk = [b as u8, g as u8, r as u8, 255];
            }
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
