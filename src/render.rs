use crate::blocks::Block;
use crate::raster::Rasterizer;
use crate::wayland::buffer::Buffer;
use crate::wayland::output::Output;
use crate::{debug, info};
use wayland_client::QueueHandle;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_shm;

// Colors are [R, G, B, A]
pub const COLOR_BACKGROUND: [u8; 4] = [0, 0, 0, 255];
pub const COLOR_ACTIVE: [u8; 4] = [255, 255, 255, 255];
pub const COLOR_INACTIVE: [u8; 4] = [100, 100, 100, 255];
pub const COLOR_URGENT: [u8; 4] = [220, 50, 50, 255];
pub const COLOR_WORKSPACE_ACTIVE_BG: [u8; 4] = [0x28, 0x55, 0x77, 0xff];

fn to_bgra(rgba: [u8; 4]) -> [u8; 4] {
    [rgba[2], rgba[1], rgba[0], rgba[3]]
}

#[derive(Clone, Copy)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
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

    pub fn clear(&mut self, color: [u8; 4]) {
        let bgra = to_bgra(color);
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

    pub fn fill_rect(&self, map: &mut Map<'_>, region: Region, color: [u8; 4]) {
        let y = region.y as usize..(region.y + region.h as i32) as usize;
        let x = region.x as usize..(region.x + region.w as i32) as usize;
        if y.is_empty() || x.is_empty() {
            return;
        }

        let bgra = to_bgra(color);
        let stride = map.width as usize;
        let (chunks, _) = map.data.as_chunks_mut::<4>();
        for row in y {
            chunks[row * stride + x.start..row * stride + x.end].fill(bgra);
        }
    }

    pub fn render_text(
        &mut self,
        map: &mut Map<'_>,
        region: Region,
        text: &str,
        ft_color: [u8; 4],
        bg_color: [u8; 4],
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
        output
            .workspace_group
            .render(self, &mut map, 0, font_size, bg_color);

        let mut y = physical_height as i32;
        let block_margin = font_size;
        for block in blocks.iter_mut() {
            y -= block.height(font_size);
            block.render(self, &mut map, y, font_size, bg_color);
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
    const FG: [u8; 4] = [255, 255, 255, 255];
    const BG: [u8; 4] = [0, 0, 0, 255];

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
}
