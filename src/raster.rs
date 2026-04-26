use fontdue::Font;
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
    pub ft_color: [u8; 4],
    pub bg_color: [u8; 4],
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
    font: Font,
    cache: HashMap<CacheKey, Bitmap>,
}

impl Rasterizer {
    pub fn new(font: Font) -> Self {
        Self {
            font,
            cache: HashMap::new(),
        }
    }

    pub fn rasterize(
        &mut self,
        c: char,
        ft_size: u32,
        ft_color: [u8; 4],
        bg_color: [u8; 4],
    ) -> &Bitmap {
        let key = CacheKey {
            c,
            ft_size,
            ft_color,
            bg_color,
        };
        self.cache.entry(key).or_insert_with(|| {
            let (metrics, bitmap) = self.font.rasterize(c, ft_size as f32);
            let mut pixels = vec![0u8; metrics.width * metrics.height * 4];
            let (chunks, _) = pixels.as_chunks_mut::<4>();
            for (chunk, alpha) in chunks.iter_mut().zip(bitmap) {
                let alpha = alpha as u32;
                let b = blend!(ft_color[2], bg_color[2], alpha);
                let g = blend!(ft_color[1], bg_color[1], alpha);
                let r = blend!(ft_color[0], bg_color[0], alpha);
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
}
