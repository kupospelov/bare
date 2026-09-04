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
        let x_start = (region.x - x).max(0) as usize;
        let x_end = (region.x + region.w as i32 - x).clamp(0, bitmap.width as i32) as usize;
        if x_start >= x_end {
            return;
        }

        let stride = self.width as usize;
        let (dst, _) = self.data.as_chunks_mut::<4>();
        let (src, _) = bitmap.pixels.as_chunks::<4>();
        for row in 0..bitmap.height {
            let px_y = y + row as i32;
            if (0..self.height as i32).contains(&px_y) {
                let dst_start = px_y as usize * stride + (x + x_start as i32) as usize;
                let src_start = row * bitmap.width;
                dst[dst_start..dst_start + x_end - x_start]
                    .copy_from_slice(&src[src_start + x_start..src_start + x_end]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;
    use std::time::Instant;

    fn bitmap(width: usize, height: usize) -> Bitmap {
        let pixels = (0..width * height)
            .flat_map(|i| [i as u8 + 1, 0, 0, 255])
            .collect();
        Bitmap {
            width,
            height,
            xmin: 0,
            ymin: 0,
            advance_width: width as f32,
            pixels,
        }
    }

    fn pixel(value: u8) -> [u8; 4] {
        if value == 0 {
            [0; 4]
        } else {
            [value, 0, 0, 255]
        }
    }

    #[test]
    fn copy_clips() {
        let mut data = [[[0; 4]; 4]; 4];
        let mut map = Mem::new(data.as_flattened_mut().as_flattened_mut(), 4);
        let bitmap = bitmap(3, 3);

        map.copy(
            Region {
                x: 1,
                y: 0,
                w: 2,
                h: 4,
            },
            &bitmap,
            -1,
            0,
        );
        map.copy(
            Region {
                x: 0,
                y: 0,
                w: 4,
                h: 4,
            },
            &bitmap,
            3,
            3,
        );

        #[rustfmt::skip]
        let expected = [
            [0, 5, 6, 0],
            [0, 8, 9, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 1],
        ];
        assert_eq!(data, expected.map(|row| row.map(pixel)));
    }

    #[test]
    fn copy_empty_intersection_with_region() {
        let mut data = [[[7; 4]; 2]; 2];
        let before = data;
        let bitmap = bitmap(2, 2);

        Mem::new(data.as_flattened_mut().as_flattened_mut(), 2).copy(
            Region {
                x: 0,
                y: 0,
                w: 2,
                h: 2,
            },
            &bitmap,
            3,
            3,
        );

        assert_eq!(data, before);
    }

    #[test]
    #[ignore = "run in release mode with --ignored --nocapture"]
    fn bench_methods() {
        fn bench(name: &str, mut operation: impl FnMut()) {
            const N: u32 = 100_000;
            let start = Instant::now();
            for _ in 0..N {
                operation();
            }
            let ns = start.elapsed().as_nanos() as f64 / N as f64;
            eprintln!("{name}: {ns:.2}ns");
        }

        let mut data = vec![0; 32 * 1920 * 4];
        let mut map = Mem::new(&mut data, 1920);
        let region = Region {
            x: 100,
            y: 0,
            w: 32,
            h: 257,
        };
        let color = Color::rgb(0x28, 0x55, 0x77);
        let bitmap = bitmap(12, 16);

        bench("fill", || {
            black_box(&mut map).fill(region, color);
        });
        bench("copy", || {
            black_box(&mut map).copy(region, &bitmap, 0, 100);
        });
        bench("clear", || {
            black_box(&mut map).clear(Range::new(0, 1920), color);
        });
    }
}
