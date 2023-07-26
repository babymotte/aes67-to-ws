pub mod poem;
pub mod sdp;
pub mod stream;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::{fmt, net::Ipv4Addr, str::FromStr};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionDescriptor {
    pub multicast_address: Ipv4Addr,
    pub multicast_port: u16,
    pub bit_depth: BitDepth,
    pub channels: u16,
    pub sample_rate: u32,
    pub packet_time: f32,
}

impl Default for SessionDescriptor {
    fn default() -> Self {
        Self {
            multicast_address: Ipv4Addr::LOCALHOST,
            multicast_port: 5004,
            bit_depth: BitDepth::L16,
            channels: 2,
            sample_rate: 44100,
            packet_time: 1.0,
        }
    }
}

impl SessionDescriptor {
    pub fn buffer_size_bytes(&self) -> u32 {
        let channels = self.channels as u32;
        let bit_depth = self.bit_depth.bits() as u32;
        self.buffer_size_frames() * bit_depth * channels
    }

    pub fn buffer_size_frames(&self) -> u32 {
        let packet_time = self.packet_time;
        let sample_rate = self.sample_rate;
        (packet_time * sample_rate as f32 / 1_000.0) as u32
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BitDepth {
    L16,
    L24,
    L32,
    FloatingPoint,
}

impl fmt::Display for BitDepth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BitDepth::L16 => write!(f, "L16"),
            BitDepth::L24 => write!(f, "L24"),
            BitDepth::L32 => write!(f, "L32"),
            BitDepth::FloatingPoint => write!(f, "Floating Point"),
        }
    }
}

impl BitDepth {
    pub fn bits(&self) -> u16 {
        match self {
            BitDepth::L16 => 16,
            BitDepth::L24 => 24,
            BitDepth::L32 => 32,
            BitDepth::FloatingPoint => 32,
        }
    }

    pub fn floating_point(&self) -> bool {
        match self {
            BitDepth::FloatingPoint => true,
            _ => false,
        }
    }
}

impl FromStr for BitDepth {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains("16") {
            Ok(BitDepth::L16)
        } else if s.contains("24") {
            Ok(BitDepth::L24)
        } else if s.contains("32") {
            Ok(BitDepth::L32)
        } else if s.to_lowercase().contains("float") {
            Ok(BitDepth::FloatingPoint)
        } else {
            Err(anyhow!("invalid bit depth: {s}"))
        }
    }
}
