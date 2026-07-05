use crate::blocks::workspaces::Workspaces;
use crate::blocks::{Block, Blocks};
use crate::color::Color;
use crate::config::{BlockConfig, Config};
use crate::font;
use crate::map::Map;
use crate::map::Mem;
use crate::raster::Rasterizer;
use crate::wayland::buffer::Buffer;
use crate::wayland::output::Output;
use crate::{debug, info};
use std::fmt;
use wayland_client::QueueHandle;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_shm;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Range {
    pub start: i32,
    pub end: i32,
}

impl Range {
    pub fn new(start: i32, end: i32) -> Self {
        Self { start, end }
    }

    pub fn contains(&self, other: Range) -> bool {
        self.start <= other.start && self.end >= other.end
    }

    pub fn overlaps(&self, other: Range) -> bool {
        self.start < other.end && other.start < self.end
    }

    pub fn union(self, other: Range) -> Range {
        Range {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn clamp(self, bounds: Range) -> Range {
        Range {
            start: self.start.max(bounds.start),
            end: self.end.min(bounds.end),
        }
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

#[derive(Default)]
pub struct BlockLayout {
    pub content: i32,
    pub height: i32,
    pub config: BlockConfig,
}

#[derive(Default)]
pub struct Layout {
    pub font_size: u32,
    pub separator: u32,
    pub blocks: Vec<BlockLayout>,
}

pub struct Renderer {
    pub rasterizer: Rasterizer,
    pub font_size: u32,
    pub bg_color: Color,
}

impl Renderer {
    pub fn new(config: &Config) -> Self {
        let fonts = font::load(&config.bar.fonts);
        let size = fonts[0].size;
        Self {
            rasterizer: Rasterizer::new(fonts),
            font_size: size,
            bg_color: config.bar.color.background,
        }
    }

    pub fn draw_block(
        &self,
        map: &mut dyn Map,
        region: Region,
        config: &BlockConfig,
        background: Color,
        border: Color,
    ) -> Region {
        let outer = Region {
            x: region.x + config.margins[3],
            y: region.y + config.margins[0],
            w: (region.w as i32 - config.margins[3] - config.margins[1]).max(0) as u32,
            h: (region.h as i32 - config.margins[0] - config.margins[2]).max(0) as u32,
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
            map.fill(inner, background);
        }
        inner
    }

    fn draw_borders(&self, map: &mut dyn Map, region: Region, borders: [i32; 4], color: Color) {
        if borders[0] > 0 {
            map.fill(
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
            map.fill(
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
            map.fill(
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
            map.fill(
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
        map: &mut dyn Map,
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

        let ascent = self.rasterizer.ascent(ft_size);
        let advance = chars
            .iter()
            .map(|c| {
                self.rasterizer
                    .rasterize(*c, ft_size, ft_color, bg_color)
                    .advance_width as i32
            })
            .sum::<i32>();

        let baseline = region.y + (region.h as i32 + ascent - 1) / 2;
        let mut x_start = region.x + (region.w as i32 - advance + 1) / 2;
        for &c in &chars {
            let bitmap = self.rasterizer.rasterize(c, ft_size, ft_color, bg_color);
            map.copy(
                region,
                bitmap,
                baseline - bitmap.ymin - bitmap.height as i32,
                x_start + bitmap.xmin,
            );
            x_start += bitmap.advance_width as i32;
        }
    }

    pub fn render_block(
        &mut self,
        map: &mut dyn Map,
        block: &dyn Block,
        region: Region,
        scale: i32,
    ) {
        let color = block.colors();
        let mut y = region.y;
        for i in 0..block.len() {
            let item = block.get(i, &self.rasterizer, scale);
            self.render_text(
                map,
                Region {
                    x: region.x,
                    y,
                    w: region.w,
                    h: item.height,
                },
                &item.text,
                color.text,
                color.background,
                item.height,
            );
            y += item.height as i32 + crate::blocks::inner_margin(item.height);
        }
    }

    pub fn render(
        &mut self,
        output_id: &ObjectId,
        output: &mut Output,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<crate::State>,
        blocks: &mut Blocks,
    ) {
        let Some(mut dirty) = output.dirty else {
            return;
        };

        debug!("Output {}: render dirty region: {}", output_id, dirty);
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
            dirty = Range::new(0, physical_height as i32);
        } else {
            dirty = dirty.clamp(Range::new(0, physical_height as i32));
        }

        let buffer = output.buffer.as_mut().unwrap();
        if !buffer.released[buffer.back] {
            info!("Output {}: skip unreleased buffer", output_id);
            return;
        }

        let prev = buffer.damage[1 - buffer.back];
        if !dirty.contains(prev) {
            buffer.copy_to_back(prev, stride as usize);
        }

        let start = buffer.back * frame_size;
        let mut map = Mem::new(&mut buffer.mmap[start..start + frame_size], physical_height);
        self.render_dirty(
            &mut map,
            output_id,
            physical_width,
            physical_height,
            scale,
            dirty,
            &output.layout,
            &mut output.workspace_group,
            blocks,
        );

        buffer.damage[buffer.back] = dirty;
        buffer.released[buffer.back] = false;
        let wl_buffer = &buffer.buffers[buffer.back];
        buffer.back = 1 - buffer.back;
        output.surface.set_buffer_scale(scale);
        output.surface.attach(Some(wl_buffer), 0, 0);
        output.surface.damage_buffer(
            0,
            dirty.start,
            physical_width as i32,
            dirty.end - dirty.start,
        );
        output.surface.commit();
        output.dirty = None;
        debug!("Output {}: rendering done", output_id);
    }

    #[allow(clippy::too_many_arguments)]
    fn render_dirty<T: std::fmt::Display>(
        &mut self,
        map: &mut dyn Map,
        output_id: T,
        physical_width: u32,
        physical_height: u32,
        scale: i32,
        dirty: Range,
        output_layout: &Layout,
        workspaces: &mut Workspaces,
        blocks: &mut Blocks,
    ) {
        map.clear(dirty, self.bg_color);

        let font_size = output_layout.font_size;
        let ws_height = workspaces.height();
        if dirty.overlaps(Range::new(0, ws_height)) {
            debug!("Output {}: rendering workspaces", output_id);
            workspaces.render(
                self,
                map,
                Region {
                    x: 0,
                    y: 0,
                    w: physical_width,
                    h: ws_height.max(0) as u32,
                },
                font_size,
                dirty,
            );
        }

        let mut y = physical_height as i32;
        for i in 0..blocks.order.len() {
            let layout = &output_layout.blocks[i];
            y -= layout.height;

            let range = Range::new(y, y + layout.height);
            if dirty.overlaps(range) {
                debug!("Output {}: rendering block {}", output_id, range);
                let block = blocks.resolve(blocks.order[i]);
                let colors = block.colors();
                let inner = self.draw_block(
                    map,
                    Region {
                        x: 0,
                        y,
                        w: physical_width,
                        h: layout.height.max(0) as u32,
                    },
                    &layout.config,
                    colors.background,
                    colors.border,
                );
                self.render_block(
                    map,
                    block,
                    Region {
                        x: inner.x,
                        y: inner.y + (inner.h as i32 - layout.content).max(0) / 2,
                        w: inner.w,
                        h: layout.content.max(0) as u32,
                    },
                    scale,
                );
            }
            y -= output_layout.separator as i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::Bitmap;
    use pretty_assertions::assert_eq;

    const SIZE: u32 = 28;
    const FT_SIZE: u32 = 13;
    const FG: Color = Color::rgb(255, 255, 255);
    const BG: Color = Color::rgb(0, 0, 0);

    #[derive(Debug, Clone, PartialEq)]
    enum Call {
        Fill { region: Region, color: Color },
        Copy { region: Region },
        Clear { range: Range, color: Color },
    }

    impl Map for Vec<Call> {
        fn fill(&mut self, region: Region, color: Color) {
            self.push(Call::Fill { region, color });
        }

        fn copy(&mut self, region: Region, _bitmap: &Bitmap, _y: i32, _x: i32) {
            self.push(Call::Copy { region });
        }

        fn clear(&mut self, range: Range, color: Color) {
            self.push(Call::Clear { range, color });
        }
    }

    fn make_renderer() -> Renderer {
        crate::log::set(crate::log::Level::Error);

        let config: Config = toml::from_str(
            r###"
            [bar]
            fonts = "Sans Bold 10px"
            "###,
        )
        .unwrap();
        Renderer::new(&config)
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

    /// Asserts that the rendered string is centered.
    ///
    /// - The difference between left and right padding must not exceed `hdiff` pixels.
    /// - The difference between top and bottom padding must not exceed `vdiff` pixels.
    fn assert_centered(r: &mut Renderer, s: &str, ft_size: u32, hdiff: i32, vdiff: i32) {
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
        r.render_text(&mut Mem::new(&mut buf, SIZE), region, s, FG, BG, ft_size);

        let (xmin, xmax, ymin, ymax) =
            glyph_bounds(&buf).unwrap_or_else(|| panic!("'{s}' rendered no pixels"));
        let left = xmin;
        let right = SIZE as i32 - 1 - xmax;
        assert!(
            (left - right).abs() <= hdiff,
            "'{s}' (ft={ft_size}): left={left}, right={right} (x=[{xmin},{xmax}])"
        );

        let above = ymin;
        let below = SIZE as i32 - 1 - ymax;
        assert!(
            (above - below).abs() <= vdiff,
            "'{s}' (ft={ft_size}): above={above}, below={below} (y=[{ymin},{ymax}])"
        );
    }

    #[test]
    fn letters_are_centered() {
        let mut r = make_renderer();
        for a in 'a'..='z' {
            assert_centered(&mut r, &a.to_string(), FT_SIZE, 2, 5);
            for b in 'a'..='z' {
                assert_centered(&mut r, &format!("{a}{b}"), FT_SIZE, 3, 5);
            }
        }

        // Highest font ymax.
        assert_centered(&mut r, "af", FT_SIZE, 2, 1);
        assert_centered(&mut r, "fb", FT_SIZE, 1, 1);

        // Something in the middle.
        assert_centered(&mut r, "ac", FT_SIZE, 1, 3);
        assert_centered(&mut r, "qb", FT_SIZE, 1, 3);

        // Lowest font ymin.
        assert_centered(&mut r, "ag", FT_SIZE, 1, 5);
        assert_centered(&mut r, "qg", FT_SIZE, 1, 5);
    }

    #[test]
    fn digits_are_centered() {
        let mut r = make_renderer();
        let ft = FT_SIZE * 2 / 3;
        for a in '0'..='9' {
            assert_centered(&mut r, &a.to_string(), FT_SIZE, 2, 1);
            for b in '0'..='9' {
                assert_centered(&mut r, &format!("{a}{b}"), FT_SIZE, 1, 1);
                for c in '0'..='9' {
                    assert_centered(&mut r, &format!("{a}{b}{c}"), ft, 2, 1);
                }
            }
        }
    }

    #[test]
    fn draw_block_applies_margins_and_borders() {
        let r = make_renderer();
        let mut buf = vec![0u8; (SIZE * SIZE * 4) as usize];
        let mut map = Mem::new(&mut buf, SIZE);
        let outer = Region {
            x: 0,
            y: 0,
            w: SIZE,
            h: SIZE,
        };
        let config = BlockConfig {
            margins: [1, 2, 3, 4],
            borders: [5, 6, 7, 8],
            height: 0,
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
        let mut map = Mem::new(&mut buf, 1);
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

    #[test]
    fn render_dirty_blocks() {
        const BAR_WIDTH: u32 = 28;
        const BAR_HEIGHT: u32 = 1080;
        const SCALE: i32 = 1;
        const SEPARATOR: u32 = 15;

        let config: Config = toml::from_str(
            r###"
            [bar]
            fonts = "Sans 10px"
            blocks = [ "time.0", "time.1" ]

            [time.1]
            format = [ "CD" ]

            [time.0]
            format = [ "AB" ]
            "###,
        )
        .unwrap();

        let mut renderer = Renderer::new(&config);
        let mut workspaces = Workspaces::new(&config.workspace, renderer.font_size);
        let mut blocks = Blocks::new(&config);
        let layout = blocks.layout(&renderer.rasterizer, SCALE, SEPARATOR);
        let font_height = renderer.rasterizer.get_default_font_size(SCALE) as i32;

        // The first block is dirty.
        let mut calls: Vec<Call> = Vec::new();
        renderer.render_dirty(
            &mut calls,
            "Output1",
            BAR_WIDTH,
            BAR_HEIGHT,
            SCALE,
            Range::new(BAR_HEIGHT as i32 - font_height, BAR_HEIGHT as i32),
            &layout,
            &mut workspaces,
            &mut blocks,
        );
        let clear = vec![Call::Clear {
            range: Range {
                start: BAR_HEIGHT as i32 - font_height,
                end: BAR_HEIGHT as i32,
            },
            color: BG,
        }];
        let render_time0 = vec![
            Call::Fill {
                region: Region {
                    x: 0,
                    y: BAR_HEIGHT as i32 - font_height,
                    w: BAR_WIDTH,
                    h: font_height as u32,
                },
                color: BG,
            },
            Call::Copy {
                region: Region {
                    x: 0,
                    y: BAR_HEIGHT as i32 - font_height,
                    w: BAR_WIDTH,
                    h: font_height as u32,
                },
            },
            Call::Copy {
                region: Region {
                    x: 0,
                    y: BAR_HEIGHT as i32 - font_height,
                    w: BAR_WIDTH,
                    h: font_height as u32,
                },
            },
        ];
        assert_eq!(calls, [clear, render_time0.clone()].concat());

        // The second block is dirty.
        let mut calls: Vec<Call> = Vec::new();
        let start = (BAR_HEIGHT - SEPARATOR) as i32 - 2 * font_height;
        renderer.render_dirty(
            &mut calls,
            "Output1",
            BAR_WIDTH,
            BAR_HEIGHT,
            SCALE,
            Range::new(start, start + font_height),
            &layout,
            &mut workspaces,
            &mut blocks,
        );
        let clear = vec![Call::Clear {
            range: Range {
                start,
                end: start + font_height,
            },
            color: BG,
        }];
        let render_time1 = vec![
            Call::Fill {
                region: Region {
                    x: 0,
                    y: start,
                    w: BAR_WIDTH,
                    h: font_height as u32,
                },
                color: BG,
            },
            Call::Copy {
                region: Region {
                    x: 0,
                    y: start,
                    w: BAR_WIDTH,
                    h: font_height as u32,
                },
            },
            Call::Copy {
                region: Region {
                    x: 0,
                    y: start,
                    w: BAR_WIDTH,
                    h: font_height as u32,
                },
            },
        ];
        assert_eq!(calls, [clear, render_time1.clone()].concat());

        // The whole bar is dirty.
        let mut calls: Vec<Call> = Vec::new();
        renderer.render_dirty(
            &mut calls,
            "Output1",
            BAR_WIDTH,
            BAR_HEIGHT,
            SCALE,
            Range::new(0, BAR_HEIGHT as i32),
            &layout,
            &mut workspaces,
            &mut blocks,
        );
        let clear = vec![Call::Clear {
            range: Range {
                start: 0,
                end: BAR_HEIGHT as i32,
            },
            color: BG,
        }];
        assert_eq!(calls, [clear, render_time0, render_time1].concat());
    }
}
