//! DMA (Direct Memory Access) controller simulation.
//!
//! Provides DMA transfer descriptors for memory-to-peripheral and
//! peripheral-to-memory transfers. Completion is signaled via IRQ callbacks.

use thiserror::Error;

/// DMA transfer direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaDirection {
    /// Memory → Peripheral.
    MemoryToPeripheral,
    /// Peripheral → Memory.
    PeripheralToMemory,
    /// Memory → Memory.
    MemoryToMemory,
}

/// DMA transfer status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaStatus {
    /// Transfer is idle / not started.
    Idle,
    /// Transfer is in progress.
    Active,
    /// Transfer completed successfully.
    Complete,
    /// Transfer encountered an error.
    Error,
}

/// DMA transfer descriptor.
#[derive(Debug, Clone)]
pub struct DmaDescriptor {
    /// Source address.
    pub src_addr: u64,
    /// Destination address.
    pub dst_addr: u64,
    /// Number of bytes to transfer.
    pub length: usize,
    /// Transfer direction.
    pub direction: DmaDirection,
    /// IRQ number to fire on completion (None = no interrupt).
    pub completion_irq: Option<u8>,
}

/// DMA channel state.
#[derive(Debug, Clone)]
struct DmaChannel {
    /// Current descriptor.
    descriptor: Option<DmaDescriptor>,
    /// Transfer status.
    status: DmaStatus,
    /// Bytes transferred so far.
    bytes_transferred: usize,
}

/// DMA errors.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum DmaError {
    /// Channel is busy with another transfer.
    #[error("DMA channel {channel} is busy")]
    ChannelBusy { channel: usize },

    /// Invalid channel number.
    #[error("invalid DMA channel: {channel} (max: {max})")]
    InvalidChannel { channel: usize, max: usize },

    /// No active transfer on this channel.
    #[error("no active transfer on DMA channel {channel}")]
    NoActiveTransfer { channel: usize },

    /// Transfer length is zero.
    #[error("DMA transfer length must be > 0")]
    ZeroLength,
}

/// Simulated DMA controller with multiple channels.
#[derive(Debug)]
pub struct DmaController {
    /// DMA channels.
    channels: Vec<DmaChannel>,
    /// Completed IRQs pending dispatch.
    pending_irqs: Vec<u8>,
}

impl DmaController {
    /// Creates a DMA controller with the specified number of channels.
    pub fn new(num_channels: usize) -> Self {
        let channels = (0..num_channels)
            .map(|_| DmaChannel {
                descriptor: None,
                status: DmaStatus::Idle,
                bytes_transferred: 0,
            })
            .collect();
        Self {
            channels,
            pending_irqs: Vec::new(),
        }
    }

    /// Returns the number of channels.
    pub fn num_channels(&self) -> usize {
        self.channels.len()
    }

    /// Starts a DMA transfer on the given channel.
    pub fn start_transfer(
        &mut self,
        channel: usize,
        descriptor: DmaDescriptor,
    ) -> Result<(), DmaError> {
        if channel >= self.channels.len() {
            return Err(DmaError::InvalidChannel {
                channel,
                max: self.channels.len() - 1,
            });
        }
        if self.channels[channel].status == DmaStatus::Active {
            return Err(DmaError::ChannelBusy { channel });
        }
        if descriptor.length == 0 {
            return Err(DmaError::ZeroLength);
        }

        self.channels[channel] = DmaChannel {
            descriptor: Some(descriptor),
            status: DmaStatus::Active,
            bytes_transferred: 0,
        };
        Ok(())
    }

    /// Simulates completing the transfer on a channel.
    ///
    /// In real hardware this would happen asynchronously. In our simulation
    /// we complete transfers explicitly.
    pub fn complete_transfer(&mut self, channel: usize) -> Result<(), DmaError> {
        if channel >= self.channels.len() {
            return Err(DmaError::InvalidChannel {
                channel,
                max: self.channels.len() - 1,
            });
        }
        let ch = &mut self.channels[channel];
        if ch.status != DmaStatus::Active {
            return Err(DmaError::NoActiveTransfer { channel });
        }

        if let Some(desc) = &ch.descriptor {
            ch.bytes_transferred = desc.length;
            if let Some(irq) = desc.completion_irq {
                self.pending_irqs.push(irq);
            }
        }
        ch.status = DmaStatus::Complete;
        Ok(())
    }

    /// Returns the status of a channel.
    pub fn channel_status(&self, channel: usize) -> Result<DmaStatus, DmaError> {
        if channel >= self.channels.len() {
            return Err(DmaError::InvalidChannel {
                channel,
                max: self.channels.len() - 1,
            });
        }
        Ok(self.channels[channel].status)
    }

    /// Returns bytes transferred on a channel.
    pub fn bytes_transferred(&self, channel: usize) -> Result<usize, DmaError> {
        if channel >= self.channels.len() {
            return Err(DmaError::InvalidChannel {
                channel,
                max: self.channels.len() - 1,
            });
        }
        Ok(self.channels[channel].bytes_transferred)
    }

    /// Resets a channel to idle state.
    pub fn reset_channel(&mut self, channel: usize) -> Result<(), DmaError> {
        if channel >= self.channels.len() {
            return Err(DmaError::InvalidChannel {
                channel,
                max: self.channels.len() - 1,
            });
        }
        self.channels[channel] = DmaChannel {
            descriptor: None,
            status: DmaStatus::Idle,
            bytes_transferred: 0,
        };
        Ok(())
    }

    /// Drains pending completion IRQs.
    pub fn drain_pending_irqs(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending_irqs)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_controller() {
        let dma = DmaController::new(4);
        assert_eq!(dma.num_channels(), 4);
    }

    #[test]
    fn start_and_complete_transfer() {
        let mut dma = DmaController::new(2);
        let desc = DmaDescriptor {
            src_addr: 0x1000,
            dst_addr: 0x2000,
            length: 256,
            direction: DmaDirection::MemoryToMemory,
            completion_irq: None,
        };
        dma.start_transfer(0, desc).unwrap();
        assert_eq!(dma.channel_status(0).unwrap(), DmaStatus::Active);

        dma.complete_transfer(0).unwrap();
        assert_eq!(dma.channel_status(0).unwrap(), DmaStatus::Complete);
        assert_eq!(dma.bytes_transferred(0).unwrap(), 256);
    }

    #[test]
    fn completion_irq_fires() {
        let mut dma = DmaController::new(1);
        let desc = DmaDescriptor {
            src_addr: 0,
            dst_addr: 0,
            length: 64,
            direction: DmaDirection::MemoryToPeripheral,
            completion_irq: Some(0x30),
        };
        dma.start_transfer(0, desc).unwrap();
        dma.complete_transfer(0).unwrap();

        let irqs = dma.drain_pending_irqs();
        assert_eq!(irqs, vec![0x30]);
    }

    #[test]
    fn no_irq_when_not_configured() {
        let mut dma = DmaController::new(1);
        let desc = DmaDescriptor {
            src_addr: 0,
            dst_addr: 0,
            length: 32,
            direction: DmaDirection::PeripheralToMemory,
            completion_irq: None,
        };
        dma.start_transfer(0, desc).unwrap();
        dma.complete_transfer(0).unwrap();

        let irqs = dma.drain_pending_irqs();
        assert!(irqs.is_empty());
    }

    #[test]
    fn busy_channel_returns_error() {
        let mut dma = DmaController::new(1);
        let desc = DmaDescriptor {
            src_addr: 0,
            dst_addr: 0,
            length: 16,
            direction: DmaDirection::MemoryToMemory,
            completion_irq: None,
        };
        dma.start_transfer(0, desc.clone()).unwrap();
        let result = dma.start_transfer(0, desc);
        assert!(matches!(result, Err(DmaError::ChannelBusy { channel: 0 })));
    }

    #[test]
    fn invalid_channel_returns_error() {
        let mut dma = DmaController::new(2);
        let desc = DmaDescriptor {
            src_addr: 0,
            dst_addr: 0,
            length: 16,
            direction: DmaDirection::MemoryToMemory,
            completion_irq: None,
        };
        assert!(matches!(
            dma.start_transfer(5, desc),
            Err(DmaError::InvalidChannel { channel: 5, max: 1 })
        ));
    }

    #[test]
    fn zero_length_returns_error() {
        let mut dma = DmaController::new(1);
        let desc = DmaDescriptor {
            src_addr: 0,
            dst_addr: 0,
            length: 0,
            direction: DmaDirection::MemoryToMemory,
            completion_irq: None,
        };
        assert!(matches!(
            dma.start_transfer(0, desc),
            Err(DmaError::ZeroLength)
        ));
    }

    #[test]
    fn complete_inactive_channel_returns_error() {
        let mut dma = DmaController::new(1);
        assert!(matches!(
            dma.complete_transfer(0),
            Err(DmaError::NoActiveTransfer { channel: 0 })
        ));
    }

    #[test]
    fn reset_channel() {
        let mut dma = DmaController::new(1);
        let desc = DmaDescriptor {
            src_addr: 0,
            dst_addr: 0,
            length: 64,
            direction: DmaDirection::MemoryToMemory,
            completion_irq: None,
        };
        dma.start_transfer(0, desc).unwrap();
        dma.complete_transfer(0).unwrap();
        dma.reset_channel(0).unwrap();
        assert_eq!(dma.channel_status(0).unwrap(), DmaStatus::Idle);
        assert_eq!(dma.bytes_transferred(0).unwrap(), 0);
    }

    #[test]
    fn multiple_channels_independent() {
        let mut dma = DmaController::new(3);
        let desc = |len| DmaDescriptor {
            src_addr: 0,
            dst_addr: 0,
            length: len,
            direction: DmaDirection::MemoryToMemory,
            completion_irq: None,
        };
        dma.start_transfer(0, desc(100)).unwrap();
        dma.start_transfer(2, desc(200)).unwrap();

        assert_eq!(dma.channel_status(0).unwrap(), DmaStatus::Active);
        assert_eq!(dma.channel_status(1).unwrap(), DmaStatus::Idle);
        assert_eq!(dma.channel_status(2).unwrap(), DmaStatus::Active);

        dma.complete_transfer(0).unwrap();
        assert_eq!(dma.channel_status(0).unwrap(), DmaStatus::Complete);
        assert_eq!(dma.channel_status(2).unwrap(), DmaStatus::Active);
    }
}
