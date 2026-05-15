use crate::blocks::Block;
use crate::color::Color;
use crate::config::BlockConfig;
use crate::raster::Rasterizer;
use crate::wayland::buffer::Buffer;
use crate::wayland::output::Output;
use crate::{debug, info};
use wayland_client::QueueHandle;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_shm;

pub const COLOR_BACKGROUND: Color = Color::rgb(0, 0, 0);
pub const COLOR_INACTIVE: Color = Color::rgb(100, 100, 100);

#[derive(Clone, Copy)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

pub struct Layout {
    pub height: i32,
    pub config: BlockConfig,
    pub background: Color,
    pub border: Color,
}

pub struct Map<'a> {
    pub data: &'a mut [u8],
    pub width: u32,
    pub height: u32,
}

impl<'a> Map<'a> {
    pub fn new(data: &'a mut [u8], height: u32) -> Self {
        let width = data.len() as u32 / 4 / height;
        Self {
            data,
            width,
            height,
        }
    }

    pub fn clear(&mut self, color: Color) {
        let bgra = color.bgra();
        let (chunks, _) = self.data.as_chunks_mut::<4>();
        chunks.fill(bgra);
    }
}

pub struct Renderer {
    pub rasterizer: Rasterizer,
    font_size: u32,
}

impl Renderer {
    pub fn new(rasterizer: Rasterizer, font_size: u32) -> Self {
        Self {
            rasterizer,
            font_size,
        }
    }

    pub fn fill_rect(&self, map: &mut Map<'_>, region: Region, color: Color) {
        let y = region.y as usize..(region.y + region.h as i32) as usize;
        let x = region.x as usize..(region.x + region.w as i32) as usize;
        if y.is_empty() || x.is_empty() {
            return;
        }

        let bgra = color.bgra();
        let stride = map.width as usize;
        let (chunks, _) = map.data.as_chunks_mut::<4>();
        for row in y {
            chunks[row * stride + x.start..row * stride + x.end].fill(bgra);
        }
    }

    pub fn draw_block(
        &self,
        map: &mut Map<'_>,
        region: Region,
        config: &BlockConfig,
        background: Color,
        border: Color,
    ) -> Region {
        let outer = Region {
            x: region.x + config.gaps[3],
            y: region.y + config.gaps[0],
            w: (region.w as i32 - config.gaps[3] - config.gaps[1]).max(0) as u32,
            h: (region.h as i32 - config.gaps[0] - config.gaps[2]).max(0) as u32,
        };
        let inner = Region {
            x: outer.x + config.borders[3],
            y: outer.y + config.borders[0],
            w: (outer.w as i32 - config.borders[3] - config.borders[1]).max(0) as u32,
            h: (outer.h as i32 - config.borders[0] - config.borders[2]).max(0) as u32,
        };
        if outer.w > 0 && outer.h > 0 {
            self.draw_borders(map, outer, config.borders, border);
        }
        if inner.w > 0 && inner.h > 0 {
            // TODO: Skip if the bar background has the same color.
            self.fill_rect(map, inner, background);
        }
        inner
    }

    fn draw_borders(&self, map: &mut Map<'_>, region: Region, borders: [i32; 4], color: Color) {
        if borders[0] > 0 {
            self.fill_rect(
                map,
                Region {
                    x: region.x,
                    y: region.y,
                    w: region.w,
                    h: borders[0] as u32,
                },
                color,
            );
        }
        if borders[1] > 0 {
            self.fill_rect(
                map,
                Region {
                    x: region.x + region.w as i32 - borders[1],
                    y: region.y,
                    w: borders[1] as u32,
                    h: region.h,
                },
                color,
            );
        }
        if borders[2] > 0 {
            self.fill_rect(
                map,
                Region {
                    x: region.x,
                    y: region.y + region.h as i32 - borders[2],
                    w: region.w,
                    h: borders[2] as u32,
                },
                color,
            );
        }
        if borders[3] > 0 {
            self.fill_rect(
                map,
                Region {
                    x: region.x,
                    y: region.y,
                    w: borders[3] as u32,
                    h: region.h,
                },
                color,
            );
        }
    }

    pub fn render_text(
        &mut self,
        map: &mut Map<'_>,
        region: Region,
        text: &str,
        ft_color: Color,
        bg_color: Color,
        ft_size: u32,
    ) {
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            return;
        }

        let mut total_advance = 0_i32;
        let mut max_ymax = i32::MIN;
        let mut min_ymin = i32::MAX;
        for &c in &chars {
            let b = self.rasterizer.rasterize(c, ft_size, ft_color, bg_color);
            total_advance += b.advance_width as i32;
            max_ymax = max_ymax.max(b.ymin + b.height as i32);
            min_ymin = min_ymin.min(b.ymin);
        }
        let first_xmin = self
            .rasterizer
            .rasterize(*chars.first().unwrap(), ft_size, ft_color, bg_color)
            .xmin;
        let text_width = {
            let last =
                self.rasterizer
                    .rasterize(*chars.last().unwrap(), ft_size, ft_color, bg_color);
            total_advance - first_xmin - last.advance_width as i32 + last.xmin + last.width as i32
        };

        let baseline = region.y + (region.h as i32 + max_ymax + min_ymin + 1) / 2;
        let stride = map.width as usize;

        let mut x_start = region.x + (region.w as i32 - text_width + 1) / 2 - first_xmin;
        let x_end = region.x + region.w as i32;

        let (dst, _) = map.data.as_chunks_mut::<4>();
        for &c in &chars {
            let bitmap = self.rasterizer.rasterize(c, ft_size, ft_color, bg_color);
            let offset_y = baseline - bitmap.ymin - bitmap.height as i32;
            let offset_x = x_start + bitmap.xmin;

            let (src, _) = bitmap.pixels.as_chunks::<4>();
            for row in 0..bitmap.height {
                for col in 0..bitmap.width {
                    let px_y = offset_y + row as i32;
                    let px_x = offset_x + col as i32;
                    if (0..map.height as i32).contains(&px_y) && (region.x..x_end).contains(&px_x) {
                        dst[px_y as usize * stride + px_x as usize] = src[row * bitmap.width + col];
                    }
                }
            }
            x_start += bitmap.advance_width as i32;
        }
    }

    pub fn render(
        &mut self,
        output_id: &ObjectId,
        output: &mut Output,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<crate::State>,
        blocks: &mut [Box<dyn Block>],
    ) {
        let logical_width = output.width;
        let logical_height = output.height;
        let scale = output.scale;
        let physical_width = logical_width * scale as u32;
        let physical_height = logical_height * scale as u32;
        let stride = physical_width * 4;
        let frame_size = (stride * physical_height) as usize;

        if output.buffer.as_ref().map_or(0, |b| b.frame_size) != frame_size {
            output.buffer = Some(Buffer::allocate(
                output_id,
                physical_width,
                physical_height,
                stride,
                frame_size,
                shm,
                qh,
            ));
        }

        let buffer = output.buffer.as_mut().unwrap();
        if !buffer.released[buffer.back] {
            info!("Output {}: skip unreleased buffer", output_id);
            return;
        }

        let bg_color = COLOR_BACKGROUND;
        let start = buffer.back * frame_size;
        let mut map = Map::new(&mut buffer.mmap[start..start + frame_size], physical_height);
        map.clear(bg_color);

        let font_size = self.font_size * scale as u32;
        let layout = output.workspace_group.layout(font_size);
        let inner = self.draw_block(
            &mut map,
            Region {
                x: 0,
                y: 0,
                w: physical_width,
                h: layout.height.max(0) as u32,
            },
            &layout.config,
            layout.background,
            layout.border,
        );
        output
            .workspace_group
            .render(self, &mut map, inner, font_size);

        let mut y = physical_height as i32;
        let block_margin = font_size;
        for block in blocks.iter_mut() {
            // TODO: Compute layouts once.
            let layout = block.layout(font_size);
            y -= layout.height;
            let inner = self.draw_block(
                &mut map,
                Region {
                    x: 0,
                    y,
                    w: physical_width,
                    h: layout.height.max(0) as u32,
                },
                &layout.config,
                layout.background,
                layout.border,
            );
            block.render(self, &mut map, inner, font_size);
            y -= block_margin as i32;
        }

        buffer.released[buffer.back] = false;
        let wl_buffer = &buffer.buffers[buffer.back];
        buffer.back = 1 - buffer.back;
        output.surface.set_buffer_scale(scale);
        output.surface.attach(Some(wl_buffer), 0, 0);
        output
            .surface
            .damage(0, 0, logical_width as i32, logical_height as i32);
        output.surface.commit();
        output.render = false;
        debug!("Output {}: rendering done", output_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font;
    use crate::raster::Rasterizer;

    const SIZE: u32 = 28;
    const FT_SIZE: u32 = 13;
    const FG: Color = Color::rgb(255, 255, 255);
    const BG: Color = Color::rgb(0, 0, 0);

    fn make_renderer() -> Renderer {
        let (font, size) = font::load("Sans Bold");
        Renderer::new(Rasterizer::new(font), size)
    }

    fn glyph_bounds(buf: &[u8]) -> Option<(i32, i32, i32, i32)> {
        let mut xmin = i32::MAX;
        let mut xmax = i32::MIN;
        let mut ymin = i32::MAX;
        let mut ymax = i32::MIN;
        for y in 0..SIZE as i32 {
            for x in 0..SIZE as i32 {
                let i = (y as usize * SIZE as usize + x as usize) * 4;
                if buf[i] != 0 || buf[i + 1] != 0 || buf[i + 2] != 0 {
                    xmin = xmin.min(x);
                    xmax = xmax.max(x);
                    ymin = ymin.min(y);
                    ymax = ymax.max(y);
                }
            }
        }
        (xmin <= xmax).then_some((xmin, xmax, ymin, ymax))
    }

    fn assert_centered(r: &mut Renderer, s: &str, ft_size: u32) {
        let mut buf = vec![0u8; (SIZE * SIZE * 4) as usize];
        for px in buf.chunks_exact_mut(4) {
            px.copy_from_slice(&[0, 0, 0, 255]);
        }
        let region = Region {
            x: 0,
            y: 0,
            w: SIZE,
            h: SIZE,
        };
        r.render_text(&mut Map::new(&mut buf, SIZE), region, s, FG, BG, ft_size);

        let (xmin, xmax, ymin, ymax) =
            glyph_bounds(&buf).unwrap_or_else(|| panic!("'{s}' rendered no pixels"));
        let above = ymin;
        let below = SIZE as i32 - 1 - ymax;
        let left = xmin;
        let right = SIZE as i32 - 1 - xmax;
        assert!(
            (above - below).abs() <= 1,
            "'{s}' (ft={ft_size}): above={above}, below={below} (y=[{ymin},{ymax}])"
        );
        assert!(
            (left - right).abs() <= 1,
            "'{s}' (ft={ft_size}): left={left}, right={right} (x=[{xmin},{xmax}])"
        );
    }

    #[test]
    fn letters_are_centered() {
        let mut r = make_renderer();
        for a in 'a'..='z' {
            assert_centered(&mut r, &a.to_string(), FT_SIZE);
            for b in 'a'..='z' {
                assert_centered(&mut r, &format!("{a}{b}"), FT_SIZE);
            }
        }
    }

    #[test]
    fn digits_are_centered() {
        let mut r = make_renderer();
        let ft = FT_SIZE * 2 / 3;
        for a in '0'..='9' {
            assert_centered(&mut r, &a.to_string(), FT_SIZE);
            for b in '0'..='9' {
                assert_centered(&mut r, &format!("{a}{b}"), FT_SIZE);
                for c in '0'..='9' {
                    assert_centered(&mut r, &format!("{a}{b}{c}"), ft);
                }
            }
        }
    }

    #[test]
    fn draw_block_applies_gaps_and_borders() {
        let r = make_renderer();
        let mut buf = vec![0u8; (SIZE * SIZE * 4) as usize];
        let mut map = Map::new(&mut buf, SIZE);
        let outer = Region {
            x: 0,
            y: 0,
            w: SIZE,
            h: SIZE,
        };
        let config = BlockConfig {
            gaps: [1, 2, 3, 4],
            borders: [5, 6, 7, 8],
        };
        let inner = r.draw_block(&mut map, outer, &config, BG, FG);
        assert_eq!(inner.x, 4 + 8);
        assert_eq!(inner.y, 1 + 5);
        assert_eq!(inner.w, SIZE - 2 - 4 - 6 - 8);
        assert_eq!(inner.h, SIZE - 1 - 3 - 5 - 7);
    }

    #[test]
    fn draw_block_zero_sized_outer_is_noop() {
        let r = make_renderer();
        let mut buf = vec![0u8; 4];
        let mut map = Map::new(&mut buf, 1);
        let outer = Region {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        };
        let inner = r.draw_block(&mut map, outer, &BlockConfig::default(), BG, FG);
        assert_eq!(inner.w, 0);
        assert_eq!(inner.h, 0);
        assert_eq!(buf, vec![0u8; 4]);
    }
}
