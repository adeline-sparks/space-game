use std::{sync::{atomic::{AtomicBool, Ordering}, Arc}};

use wgpu::{Buffer, Device, BufferDescriptor, BufferUsages, BufferView, MapMode};

pub struct DownloadQueue {
    buffers: Box<[Buffer]>,
    next_buffer: usize,
    mapped_flags: Arc<[AtomicBool]>,
    next_flag: usize,
    possibly_full: bool,
}

impl DownloadQueue {
    pub fn new(device: &Device, size: usize, depth: usize) -> DownloadQueue {
        DownloadQueue { 
            buffers: (0..depth)
                .into_iter().map(|_| 
                    device.create_buffer(&BufferDescriptor {
                        label: None,
                        size: size as u64,
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
                ).collect::<Vec<_>>()
                .into(),
            next_buffer: 0, 
            mapped_flags: (0..depth)
                .into_iter()
                .map(|_| AtomicBool::default())
                .collect::<Vec<_>>()
                .into(), 
            next_flag: 0,
            possibly_full: false,
        }
    }

    pub fn empty(&self) -> bool {
        (self.next_buffer == self.next_flag) && !self.possibly_full
    }

    pub fn full(&self) -> bool {
        (self.next_buffer == self.next_flag) && self.possibly_full
    }

    pub fn mapped_buffer_view(&self) -> Option<BufferView> {
        self.mapped_flags[self.next_flag].load(Ordering::Acquire).then(|| {
            self.buffers[self.next_flag].slice(..).get_mapped_range()
        })
    }

    pub fn unmap_buffer(&mut self) {
        if !self.mapped_flags[self.next_flag].swap(false, Ordering::Acquire) {
            panic!("pop on empty queue");
        }
        self.buffers[self.next_flag].unmap();
        self.next_flag = (self.next_flag + 1) % self.buffers.len();
        self.possibly_full = false;
    }

    pub fn active_buffer(&self) -> Option<&Buffer> {
        (!self.full()).then_some(&self.buffers[self.next_buffer])
    }

    pub fn map_active_buffer(&mut self) {
        if self.full() { 
            return;
        }

        let mapped_flags = self.mapped_flags.clone();
        let next_buffer = self.next_buffer;

        self.buffers[next_buffer].slice(..).map_async(MapMode::Read, move |result| {
            if result.is_err() {
                todo!();
            }

            if mapped_flags[next_buffer].swap(true, Ordering::Release) {
                todo!();
            }
        });

        self.next_buffer = (self.next_buffer + 1) % self.buffers.len();
        self.possibly_full = true;
    }
}