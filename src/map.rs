use crate::color::Color;
use crate::raster::Bitmap;
use crate::render::{Range, Region};

#[derive(Debug, Clone, Copy)]
pub struct Flip {
    pub h: bool,
    pub v: bool,
}

pub trait Map {
    fn fill(&mut self, region: Region, color: Color);
    fn copy(&mut self, region: Region, bitmap: &Bitmap, y: i32, x: i32, flip: Flip);
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

    fn copy(&mut self, region: Region, bitmap: &Bitmap, y: i32, x: i32, flip: Flip) {
        let dy1 = y.max(region.y).max(0);
        let dy2 = (y + bitmap.height as i32)
            .min(region.y + region.h as i32)
            .min(self.height as i32);
        let dx1 = x.max(region.x).max(0);
        let dx2 = (x + bitmap.width as i32)
            .min(region.x + region.w as i32)
            .min(self.width as i32);
        if dx1 >= dx2 || dy1 >= dy2 {
            return;
        }

        let width = (dx2 - dx1) as usize;
        let sx = if flip.h {
            bitmap.width - (dx2 - x) as usize
        } else {
            (dx1 - x) as usize
        };
        let (dst, _) = self.data.as_chunks_mut::<4>();
        let (src, _) = bitmap.pixels.as_chunks::<4>();
        for dy in dy1..dy2 {
            let sy = if flip.v {
                bitmap.height - 1 - (dy - y) as usize
            } else {
                (dy - y) as usize
            };
            let dst_start = dy as usize * self.width as usize + dx1 as usize;
            let src_start = sy * bitmap.width + sx;
            let dst = &mut dst[dst_start..dst_start + width];
            let src = &src[src_start..src_start + width];
            if flip.h {
                dst.iter_mut()
                    .zip(src.iter().rev())
                    .for_each(|(dst, src)| *dst = *src);
            } else {
                dst.copy_from_slice(src);
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
    use crate::tests::util::bench;
    use std::hint::black_box;

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
        let bitmap = bitmap(3, 3);

        #[rustfmt::skip]
        let cases = [
            (Flip { h: false, v: false }, [
                [0, 5, 6, 0],
                [0, 8, 9, 0],
                [0, 0, 0, 0],
                [0, 0, 0, 1],
            ]),
            (Flip { h: true, v: false }, [
                [0, 5, 4, 0],
                [0, 8, 7, 0],
                [0, 0, 0, 0],
                [0, 0, 0, 3],
            ]),
            (Flip { h: false, v: true }, [
                [0, 5, 6, 0],
                [0, 2, 3, 0],
                [0, 0, 0, 0],
                [0, 0, 0, 7],
            ]),
            (Flip { h: true, v: true }, [
                [0, 5, 4, 0],
                [0, 2, 1, 0],
                [0, 0, 0, 0],
                [0, 0, 0, 9],
            ]),
        ];

        for (flip, expected) in cases {
            let mut data = [[[0; 4]; 4]; 4];
            let mut map = Mem::new(data.as_flattened_mut().as_flattened_mut(), 4);

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
                flip,
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
                flip,
            );

            assert_eq!(data, expected.map(|row| row.map(pixel)), "flip={:?}", flip);
        }
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
            Flip { h: false, v: false },
        );

        assert_eq!(data, before);
    }

    #[test]
    #[ignore = "run in release mode with --ignored --nocapture"]
    fn bench_copy() {
        for size in [8, 12, 16, 32, 64, 256] {
            let width = (size + 8).max(32);
            let mut data = vec![0; width * 1920 * 4];
            let mut map = Mem::new(&mut data, 1920);
            let bitmap = bitmap(size, size);
            let region = Region {
                x: 0,
                y: 0,
                w: width as u32,
                h: 1920,
            };
            for (label, y, x) in [("full", 10, 4), ("clipped", -3, -3)] {
                for (name, flip) in [
                    ("none", Flip { h: false, v: false }),
                    ("h", Flip { h: true, v: false }),
                    ("v", Flip { h: false, v: true }),
                    ("hv", Flip { h: true, v: true }),
                ] {
                    bench(&format!("{size}x{size}/{label}/{name}"), || {
                        black_box(&mut map).copy(
                            black_box(region),
                            black_box(&bitmap),
                            black_box(y),
                            black_box(x),
                            black_box(flip),
                        );
                    });
                }
            }
        }
    }

    #[test]
    #[ignore = "run in release mode with --ignored --nocapture"]
    fn bench_fill() {
        let mut data = vec![0; 32 * 1920 * 4];
        let mut map = Mem::new(&mut data, 1920);
        let region = Region {
            x: 0,
            y: 0,
            w: 32,
            h: 257,
        };
        let color = Color::rgb(0x28, 0x55, 0x77);

        bench("fill", || {
            black_box(&mut map).fill(region, color);
        });
        bench("clear", || {
            black_box(&mut map).clear(Range::new(0, 1920), color);
        });
    }
}
