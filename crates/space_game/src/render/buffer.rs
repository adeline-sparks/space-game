use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use wgpu::{Buffer, BufferDescriptor, BufferUsages, BufferView, Device, MapMode, BufferViewMut};

pub struct StagingBuffer {
    buffer: Buffer,
    mode: MapMode,
    map_requested: bool,
    map_complete: Arc<AtomicBool>,
}

impl StagingBuffer {
    pub fn new_read(device: &Device, size: usize) -> Self {
        Self::new(device, size, MapMode::Read, false)
    }
    pub fn new(device: &Device, size: usize, mode: MapMode, mapped_at_creation: bool) -> Self {
        let usage = match mode {
            MapMode::Read => BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            MapMode::Write => BufferUsages::COPY_SRC | BufferUsages::MAP_WRITE,
        };

        let buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: size as u64,
            usage,
            mapped_at_creation,
        });

        StagingBuffer { 
            buffer, 
            mode, 
            map_requested: mapped_at_creation,
            map_complete: Arc::new(AtomicBool::new(mapped_at_creation)),
        }
    }

    pub fn try_buffer(&self) -> Option<&Buffer> {
        (!self.map_requested).then_some(&self.buffer)
    }

    pub fn map_async(&mut self) {
        if self.map_requested {
            return;
        }
        self.map_requested = true;

        let map_complete = Arc::clone(&self.map_complete);
        self.buffer.slice(..).map_async(MapMode::Read, move |result| {
            assert!(result.is_ok());
            map_complete.store(true, Ordering::Release);
        })
    }

    pub fn try_view(&self) -> Option<BufferView> {
        self.map_complete
            .load(Ordering::Acquire)
            .then(|| self.buffer.slice(..).get_mapped_range())
    }

    pub fn try_view_mut(&mut self) -> Option<BufferViewMut> {
        assert!(self.mode == MapMode::Write);
        self.map_complete
            .load(Ordering::Acquire)
            .then(|| self.buffer.slice(..).get_mapped_range_mut())        
    }

    pub fn unmap(&mut self) {
        assert!(self.map_complete.load(Ordering::Relaxed));

        self.buffer.unmap();
        self.map_requested = false;
        self.map_complete.store(false, Ordering::Relaxed);
    }
}