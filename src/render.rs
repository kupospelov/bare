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
pub const FONT_SIZE: u32 = 8;

fn to_bgra(rgba: [u8; 4]) -> [u8; 4] {
    [rgba[2], rgba[1], rgba[0], rgba[3]]
}

fn ft_size_px(scale: i32) -> u32 {
    FONT_SIZE * scale as u32 * 96 / 72
}

pub struct Renderer {
    pub rasterizer: Rasterizer,
}

impl Renderer {
    pub fn new(rasterizer: Rasterizer) -> Self {
        Self { rasterizer }
    }

    pub fn fill_rect(
        &self,
        mapping: &mut [u8],
        width: u32,
        height: u32,
        y: i32,
        rect_height: i32,
        color: [u8; 4],
    ) {
        let y_start = y.max(0) as usize;
        let y_end = (y + rect_height).clamp(0, height as i32) as usize;
        if y_start >= y_end {
            return;
        }

        let bgra = to_bgra(color);
        let (chunks, _) = mapping.as_chunks_mut::<4>();
        let row_len = width as usize;
        chunks[y_start * row_len..y_end * row_len].fill(bgra);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_text(
        &mut self,
        mapping: &mut [u8],
        width: u32,
        height: u32,
        y: i32,
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
        let (last_xmin, last_width, last_advance) = {
            let b = self
                .rasterizer
                .rasterize(*chars.last().unwrap(), ft_size, ft_color, bg_color);
            (b.xmin, b.width as i32, b.advance_width as i32)
        };

        let text_width = total_advance - first_xmin - last_advance + last_xmin + last_width;
        let baseline = y + (ft_size as i32 + max_ymax + min_ymin + 1) / 2;
        let mut x = (width as i32 - text_width + 1) / 2 - first_xmin;
        let (dst, _) = mapping.as_chunks_mut::<4>();
        for &c in &chars {
            let bitmap = self.rasterizer.rasterize(c, ft_size, ft_color, bg_color);
            let (src, _) = bitmap.pixels.as_chunks::<4>();

            for row in 0..bitmap.height {
                for col in 0..bitmap.width {
                    let px_y = baseline - bitmap.ymin - bitmap.height as i32 + row as i32;
                    let px_x = x + col as i32 + bitmap.xmin;
                    if px_y >= 0 && px_y < height as i32 && px_x >= 0 && px_x < width as i32 {
                        dst[px_y as usize * width as usize + px_x as usize] =
                            src[row * bitmap.width + col];
                    }
                }
            }
            x += bitmap.advance_width as i32;
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
        let bg_pixel = to_bgra(bg_color);
        let start = buffer.back * frame_size;
        let mapping = &mut buffer.mmap[start..start + frame_size];

        let (chunks, _) = mapping.as_chunks_mut::<4>();
        chunks.fill(bg_pixel);

        let font_size = ft_size_px(scale);
        output.workspace_group.render(
            self,
            mapping,
            physical_width,
            physical_height,
            0,
            font_size,
            bg_color,
        );

        let mut y = physical_height as i32;
        let block_margin = font_size;
        for block in blocks.iter_mut() {
            y -= block.height(font_size);
            block.render(
                self,
                mapping,
                physical_width,
                physical_height,
                y,
                font_size,
                bg_color,
            );
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
    use crate::raster::Rasterizer;

    const FONT_PATH: &str = "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf";
    const SIZE: u32 = 28;
    const FT_SIZE: u32 = 13;
    const FG: [u8; 4] = [255, 255, 255, 255];
    const BG: [u8; 4] = [0, 0, 0, 255];

    fn make_renderer() -> Option<Renderer> {
        let bytes = std::fs::read(FONT_PATH).ok()?;
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()?;
        Some(Renderer::new(Rasterizer::new(font)))
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
        let y = (SIZE as i32 - ft_size as i32) / 2;
        let mut buf = vec![0u8; (SIZE * SIZE * 4) as usize];
        for px in buf.chunks_exact_mut(4) {
            px.copy_from_slice(&[0, 0, 0, 255]);
        }
        r.render_text(&mut buf, SIZE, SIZE, y, s, FG, BG, ft_size);

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
        let Some(mut r) = make_renderer() else {
            return;
        };
        for a in 'a'..='z' {
            assert_centered(&mut r, &a.to_string(), FT_SIZE);
            for b in 'a'..='z' {
                assert_centered(&mut r, &format!("{a}{b}"), FT_SIZE);
            }
        }
    }

    #[test]
    fn digits_are_centered() {
        let Some(mut r) = make_renderer() else {
            return;
        };
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
