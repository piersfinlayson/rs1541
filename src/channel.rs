use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Dedicated load channel on disk drive
pub const CBM_CHANNEL_LOAD: u8 = 0;

/// Dedicated control/command channel on disk drive
pub const CBM_CHANNEL_CTRL: u8 = 15;

/// Represents a channel to a CBM drive
///
/// Channels are the primary means of communication with CBM drives. Each drive
/// supports 16 channels (0-15), with channel 15 reserved for control operations.
#[derive(Debug, Clone)]
pub struct CbmChannel {
    _number: u8,
    _purpose: CbmChannelPurpose,
}

/// Purpose for which a channel is being used
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CbmChannelPurpose {
    Reset,     // Channel 15 - reserved for reset commands
    Directory, // Reading directory
    FileRead,  // Reading a file
    FileWrite, // Writing a file
    Command,   // Other command channel operations
}

/// Manages channel allocation for a drive unit
///
/// Ensures proper allocation and deallocation of channels, maintaining
/// the invariant that channel 15 is only used for reset operations.
#[derive(Debug)]
pub struct CbmChannelManager {
    channels: HashMap<u8, Option<CbmChannel>>,
    next_sequence: AtomicU64,
}

impl CbmChannelManager {
    pub fn new() -> Self {
        let mut channels = HashMap::new();
        for i in 0..=15 {
            channels.insert(i, None);
        }
        Self {
            channels,
            next_sequence: AtomicU64::new(1), // Start at 1 to avoid handle 0
        }
    }

    /// Allocates a channel for a specific purpose
    ///
    /// Returns (channel_number, handle) if successful, None if no channels available
    /// or if attempting to allocate channel 15 for non-reset purposes
    pub fn allocate(
        &mut self,
        _device_number: u8,
        _drive_id: u8,
        purpose: CbmChannelPurpose,
    ) -> Option<u8> {
        // Channel 15 handling
        if purpose == CbmChannelPurpose::Reset {
            if let Some(slot) = self.channels.get_mut(&15) {
                if slot.is_none() {
                    let _sequence = self.next_sequence.fetch_add(1, Ordering::SeqCst);
                    *slot = Some(CbmChannel {
                        _number: 15,
                        _purpose: purpose,
                    });
                    return Some(15);
                }
            }
            return None;
        }

        // Regular channel allocation
        for i in 0..15 {
            if let Some(slot) = self.channels.get_mut(&i) {
                if slot.is_none() {
                    let _sequence = self.next_sequence.fetch_add(1, Ordering::SeqCst);
                    *slot = Some(CbmChannel {
                        _number: i,
                        _purpose: purpose,
                    });
                    return Some(i);
                }
            }
        }
        None
    }

    pub fn reset(&mut self) {
        for i in 0..=15 {
            self.channels.insert(i, None);
        }
    }
}
