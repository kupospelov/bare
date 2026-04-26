use crate::{debug, info};
use nix::sys::memfd::{MFdFlags, memfd_create};
use std::fs::File;
use std::os::fd::AsFd;
use wayland_client::QueueHandle;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::{wl_buffer, wl_shm, wl_shm_pool::WlShmPool};

pub struct Buffer {
    pub _pool: WlShmPool,
    pub buffers: [wl_buffer::WlBuffer; 2],
    pub released: [bool; 2],
    pub mmap: memmap2::MmapMut,
    pub frame_size: usize,
    pub back: usize,
}

impl Buffer {
    pub fn allocate(
        output_id: &ObjectId,
        physical_width: u32,
        physical_height: u32,
        stride: u32,
        frame_size: usize,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<crate::State>,
    ) -> Self {
        info!("Output {}: update frame size to {}", output_id, frame_size);

        let fd = memfd_create(c"wayland-shm", MFdFlags::empty()).unwrap();
        let file: File = fd.into();
        file.set_len((frame_size * 2) as u64).unwrap();

        let pool = shm.create_pool(file.as_fd(), (frame_size * 2) as i32, qh, ());
        let buf0 = pool.create_buffer(
            0,
            physical_width as i32,
            physical_height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            (output_id.clone(), 0usize),
        );
        let buf1 = pool.create_buffer(
            frame_size as i32,
            physical_width as i32,
            physical_height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            (output_id.clone(), 1usize),
        );

        let mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
        Self {
            _pool: pool,
            buffers: [buf0, buf1],
            released: [true, true],
            mmap,
            frame_size,
            back: 0,
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        debug!("Buffer frame_size={}: destroy", self.frame_size);

        for b in &self.buffers {
            b.destroy();
        }
        self._pool.destroy();
    }
}
