use std::fmt;

use serde::{Deserialize, Serialize};
use strategy_sim::MapIdentity;

use crate::StateDelta;

pub const PROTOCOL_VERSION: u16 = 3;
pub type ResumeToken = [u8; 16];

/// A private token issued by the host when reserving a player's slot.
pub fn new_resume_token() -> std::io::Result<ResumeToken> {
    let mut token = [0; 16];
    getrandom::fill(&mut token).map_err(|error| std::io::Error::other(error.to_string()))?;
    Ok(token)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LobbyController {
    Human { player_id: u32 },
    Ai { profile: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyCountry {
    pub id: u32,
    pub tag: String,
    pub name: String,
    pub controller: LobbyController,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientMessage {
    Hello {
        version: u16,
        name: String,
        map_version: u32,
        map_hash: [u8; 32],
        resume: Option<ResumeToken>,
    },
    Command {
        seq: u64,
        command: crate::PlayerCommand,
    },
    PauseRequest {
        seq: u64,
        paused: bool,
    },
    Ping {
        nonce: u64,
    },
    Resync,
    ClaimCountry {
        tag: String,
    },
    SetController {
        tag: String,
        player_id: Option<u32>,
    },
    Confirm {
        resume: ResumeToken,
    },
    Pong {
        nonce: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMessage {
    Welcome {
        player_id: u32,
        resume: ResumeToken,
    },
    Lobby {
        room: String,
        countries: Vec<LobbyCountry>,
        guest_tag: String,
    },
    Snapshot {
        sequence: u64,
        tick: u64,
        state: Vec<u8>,
    },
    Delta {
        tick: u64,
        changes: StateDelta,
    },
    Checksum {
        sequence: u64,
        tick: u64,
        hash: [u8; 32],
    },
    CommandAccepted {
        seq: u64,
    },
    CommandRejected {
        seq: u64,
        reason: String,
    },
    Rejected {
        reason: String,
    },
    Pause {
        paused: bool,
    },
    Speed {
        speed: u8,
    },
    Pong {
        nonce: u64,
    },
    Ping {
        nonce: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandshakeError {
    ProtocolVersion { received: u16 },
    MapVersion { expected: u32, received: u32 },
    MapHash,
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProtocolVersion { received } => {
                write!(f, "LAN protocol version {received} does not match host version {PROTOCOL_VERSION}")
            }
            Self::MapVersion { expected, received } => {
                write!(
                    f,
                    "map version {received} does not match host version {expected}"
                )
            }
            Self::MapHash => f.write_str("map data hash does not match the host"),
        }
    }
}

impl std::error::Error for HandshakeError {}

pub fn validate_hello(
    version: u16,
    map_version: u32,
    map_hash: [u8; 32],
    expected: MapIdentity,
) -> Result<(), HandshakeError> {
    if version != PROTOCOL_VERSION {
        return Err(HandshakeError::ProtocolVersion { received: version });
    }
    let expected_version = expected.version.get();
    if map_version != expected_version {
        return Err(HandshakeError::MapVersion {
            expected: expected_version,
            received: map_version,
        });
    }
    if map_hash != expected.sha256 {
        return Err(HandshakeError::MapHash);
    }
    Ok(())
}
