use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use wgpu::{Buffer, BufferDescriptor, BufferUsages, BufferView, Device, MapMode};

pub struct DownloadQueue {
    buffers: Box<[Buffer]>,
    write_buffer: usize,
    mapped_flags: Arc<[AtomicBool]>,
    read_buffer: usize,
    possibly_full: bool,
}

impl DownloadQueue {
    pub fn new(device: &Device, size: usize, depth: usize) -> DownloadQueue {
        DownloadQueue {
            buffers: (0..depth)
                .into_iter()
                .map(|_| {
                    device.create_buffer(&BufferDescriptor {
                        label: None,
                        size: size as u64,
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    })
                })
                .collect::<Vec<_>>()
                .into(),
            write_buffer: 0,
            mapped_flags: (0..depth)
                .into_iter()
                .map(|_| AtomicBool::default())
                .collect::<Vec<_>>()
                .into(),
            read_buffer: 0,
            possibly_full: false,
        }
    }

    pub fn empty(&self) -> bool {
        (self.write_buffer == self.read_buffer) && !self.possibly_full
    }

    pub fn full(&self) -> bool {
        (self.write_buffer == self.read_buffer) && self.possibly_full
    }

    pub fn try_read_view(&self) -> Option<BufferView> {
        self.mapped_flags[self.read_buffer]
            .load(Ordering::Acquire)
            .then(|| self.buffers[self.read_buffer].slice(..).get_mapped_range())
    }

    pub fn pop_read_view(&mut self) {
        if self.empty() {
            return;
        }

        self.buffers[self.read_buffer].unmap();
        self.mapped_flags[self.read_buffer].store(false, Ordering::Relaxed);
        self.read_buffer = (self.read_buffer + 1) % self.buffers.len();
        self.possibly_full = false;
    }

    pub fn pop_all(&mut self, mut f: impl FnMut(BufferView)) {
        loop {
            let view = self.try_read_view();
            if view.is_none() {
                break;
            }

            f(view.unwrap());
            self.pop_read_view();
        }
    }

    pub fn try_write_buffer(&self) -> Option<&Buffer> {
        (!self.full()).then_some(&self.buffers[self.write_buffer])
    }

    pub fn push_write_buffer(&mut self) {
        if self.full() {
            return;
        }

        let mapped_flags = self.mapped_flags.clone();
        let write_buffer = self.write_buffer;

        self.buffers[write_buffer]
            .slice(..)
            .map_async(MapMode::Read, move |result| {
                if result.is_err() {
                    todo!();
                }

                mapped_flags[write_buffer].store(true, Ordering::Release);
            });

        self.write_buffer = (self.write_buffer + 1) % self.buffers.len();
        self.possibly_full = true;
    }
}
