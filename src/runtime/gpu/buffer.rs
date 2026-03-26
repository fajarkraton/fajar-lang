//! GPU buffer — device memory handle with type-safe data transfer.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_BUFFER_ID: AtomicU64 = AtomicU64::new(1);

/// Opaque handle to device memory.
///
/// Buffers are created via `GpuDevice::create_buffer` and hold
/// a unique ID for backend tracking. The actual device memory is
/// managed by the backend implementation.
#[derive(Debug)]
pub struct GpuBuffer {
    /// Unique buffer identifier.
    id: u64,
    /// Size in bytes.
    size: usize,
    /// Backend-specific opaque data (e.g., wgpu::Buffer handle index).
    backend_data: BackendData,
}

/// Backend-specific storage for buffer handles.
#[derive(Debug)]
pub enum BackendData {
    /// No backend data (CPU fallback stores data in a Vec).
    CpuData(Vec<u8>),
    /// Index into backend's buffer table.
    Handle(u64),
}

impl GpuBuffer {
    /// Create a new buffer with the given size and backend data.
    pub fn new(size: usize, backend_data: BackendData) -> Self {
        GpuBuffer {
            id: NEXT_BUFFER_ID.fetch_add(1, Ordering::Relaxed),
            size,
            backend_data,
        }
    }

    /// Get the unique buffer ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get the buffer size in bytes.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get a reference to the backend data.
    pub fn backend_data(&self) -> &BackendData {
        &self.backend_data
    }

    /// Get a mutable reference to the backend data.
    pub fn backend_data_mut(&mut self) -> &mut BackendData {
        &mut self.backend_data
    }
}

impl BackendData {
    /// Get CPU data if this is a CPU fallback buffer.
    pub fn as_cpu_data(&self) -> Option<&[u8]> {
        match self {
            BackendData::CpuData(data) => Some(data),
            _ => None,
        }
    }

    /// Get mutable CPU data if this is a CPU fallback buffer.
    pub fn as_cpu_data_mut(&mut self) -> Option<&mut Vec<u8>> {
        match self {
            BackendData::CpuData(data) => Some(data),
            _ => None,
        }
    }

    /// Get the handle index if this is a backend buffer.
    pub fn as_handle(&self) -> Option<u64> {
        match self {
            BackendData::Handle(h) => Some(*h),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_unique_ids() {
        let b1 = GpuBuffer::new(1024, BackendData::CpuData(vec![0; 1024]));
        let b2 = GpuBuffer::new(2048, BackendData::CpuData(vec![0; 2048]));
        assert_ne!(b1.id(), b2.id());
        assert_eq!(b1.size(), 1024);
        assert_eq!(b2.size(), 2048);
    }

    #[test]
    fn cpu_data_access() {
        let mut buf = GpuBuffer::new(4, BackendData::CpuData(vec![1, 2, 3, 4]));
        assert_eq!(buf.backend_data().as_cpu_data(), Some(&[1u8, 2, 3, 4][..]));

        if let Some(data) = buf.backend_data_mut().as_cpu_data_mut() {
            data[0] = 10;
        }
        assert_eq!(buf.backend_data().as_cpu_data().map(|d| d[0]), Some(10));
    }

    #[test]
    fn handle_data_access() {
        let buf = GpuBuffer::new(256, BackendData::Handle(42));
        assert_eq!(buf.backend_data().as_handle(), Some(42));
        assert_eq!(buf.backend_data().as_cpu_data(), None);
    }
}
