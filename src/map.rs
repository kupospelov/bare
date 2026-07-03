use crate::color::Color;
use crate::raster::Bitmap;
use crate::render::{Range, Region};

pub trait Map {
    fn fill(&mut self, region: Region, color: Color);
    fn copy(&mut self, region: Region, bitmap: &Bitmap, y: i32, x: i32);
    fn clear(&mut self, range: Range, color: Color);
}

pub struct Mem<'a> {
    pub data: &'a mut [u8],
    pub width: u32,
    pub height: u32,
}

impl<'a> Mem<'a> {
    pub fn new(data: &'a mut [u8], height: u32) -> Self {
        let width = data.len() as u32 / 4 / height;
        Self {
            data,
            width,
            height,
        }
    }
}

impl<'a> Map for Mem<'a> {
    fn fill(&mut self, region: Region, color: Color) {
        let y = region.y as usize..(region.y + region.h as i32) as usize;
        let x = region.x as usize..(region.x + region.w as i32) as usize;
        if y.is_empty() || x.is_empty() {
            return;
        }

        let bgra = color.bgra();
        let stride = self.width as usize;
        let (chunks, _) = self.data.as_chunks_mut::<4>();
        for row in y {
            chunks[row * stride + x.start..row * stride + x.end].fill(bgra);
        }
    }

    fn copy(&mut self, region: Region, bitmap: &Bitmap, y: i32, x: i32) {
        let stride = self.width as usize;
        let x_end = region.x + region.w as i32;
        let (dst, _) = self.data.as_chunks_mut::<4>();
        let (src, _) = bitmap.pixels.as_chunks::<4>();
        for row in 0..bitmap.height {
            for col in 0..bitmap.width {
                let px_y = y + row as i32;
                let px_x = x + col as i32;
                if (0..self.height as i32).contains(&px_y) && (region.x..x_end).contains(&px_x) {
                    dst[px_y as usize * stride + px_x as usize] = src[row * bitmap.width + col];
                }
            }
        }
    }

    fn clear(&mut self, range: Range, color: Color) {
        if range.end <= range.start {
            return;
        }
        let stride = self.width as usize;
        let bgra = color.bgra();
        let (chunks, _) = self.data.as_chunks_mut::<4>();
        chunks[range.start as usize * stride..range.end as usize * stride].fill(bgra);
    }
}
