//! Host-authoritative LAN protocol, transport, discovery, and replication.

mod command;
mod discovery;
mod frame;
mod protocol;
mod replica;

pub use command::{CommandDecodeError, PlayerCommand};
pub use discovery::{
    advertise_room, discover_rooms, listen_ipv4_addresses, DiscoveredRoom, DiscoveryError,
    RoomAdvertisement, SERVICE_TYPE,
};
pub use frame::{
    read_client, read_server, write_client, write_server, MAX_CLIENT_FRAME_BYTES,
    MAX_SERVER_FRAME_BYTES,
};
pub use protocol::{
    new_resume_token, validate_hello, ClientMessage, HandshakeError, LobbyController, LobbyCountry,
    ResumeToken, ServerMessage, PROTOCOL_VERSION,
};
pub use replica::{
    canonical_checksum, make_delta, ByteEdit, Replica, ReplicaError, StateDelta, MAX_STATE_BYTES,
};
