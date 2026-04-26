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
        let baseline = y + ft_size as i32;
        let chars: Vec<char> = text.chars().collect();
        let total_width: i32 = chars
            .iter()
            .map(|&c| {
                self.rasterizer
                    .rasterize(c, ft_size, ft_color, bg_color)
                    .advance_width as i32
            })
            .sum();

        let mut x = (width as i32 - total_width) / 2;
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
