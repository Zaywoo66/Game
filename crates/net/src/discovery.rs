use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, io,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::Duration,
};

use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::time::{timeout_at, Instant};

use crate::{new_resume_token, PROTOCOL_VERSION};

pub const SERVICE_TYPE: &str = "_game._tcp.local.";
const MAX_ROOM_BYTES: usize = 100;
const MAX_PLAYERS: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredRoom {
    pub room: String,
    pub version: u16,
    pub player_count: u8,
    pub addresses: Vec<SocketAddr>,
}

#[derive(Debug)]
pub enum DiscoveryError {
    Io(io::Error),
    Mdns(mdns_sd::Error),
    InvalidRoom,
    InvalidPlayerCount(u8),
    NoIpv4Address,
    BrowseClosed,
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::Mdns(error) => error.fmt(f),
            Self::InvalidRoom => {
                f.write_str("room name must have 1–100 UTF-8 bytes and no control characters")
            }
            Self::InvalidPlayerCount(count) => {
                write!(f, "LAN room player count {count} is outside the range 1–2")
            }
            Self::NoIpv4Address => {
                f.write_str("no LAN IPv4 address is available to advertise; use manual IP entry")
            }
            Self::BrowseClosed => f.write_str("LAN discovery stopped before the search completed"),
        }
    }
}

impl std::error::Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Mdns(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for DiscoveryError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<mdns_sd::Error> for DiscoveryError {
    fn from(error: mdns_sd::Error) -> Self {
        Self::Mdns(error)
    }
}

struct Daemon(ServiceDaemon);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.shutdown();
    }
}

/// Keeps the service announced until dropped.
pub struct RoomAdvertisement {
    daemon: Daemon,
    instance: String,
    fullname: String,
    room: String,
    addresses: Vec<SocketAddr>,
    player_count: u8,
}

impl RoomAdvertisement {
    /// Concrete LAN endpoints advertised for the TCP listener.
    pub fn addresses(&self) -> &[SocketAddr] {
        &self.addresses
    }

    pub fn set_player_count(&mut self, player_count: u8) -> Result<(), DiscoveryError> {
        if player_count != self.player_count {
            let info =
                room_service_info(&self.instance, &self.room, &self.addresses, player_count)?;
            self.daemon.0.register(info)?;
            self.player_count = player_count;
        }
        Ok(())
    }
}

impl Drop for RoomAdvertisement {
    fn drop(&mut self) {
        let _ = self.daemon.0.unregister(&self.fullname);
    }
}

/// Returns concrete addresses served by an already-bound IPv4 TCP listener.
/// Wildcard listeners list active non-loopback interfaces when available.
pub fn listen_ipv4_addresses(bound: SocketAddr) -> io::Result<Vec<SocketAddr>> {
    let SocketAddr::V4(bound) = bound else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "LAN discovery requires an IPv4 TCP listener",
        ));
    };
    if bound.port() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "LAN discovery requires the assigned listener port",
        ));
    }
    if !bound.ip().is_unspecified() {
        return Ok(vec![SocketAddr::V4(bound)]);
    }

    let addresses: BTreeSet<_> = if_addrs::get_if_addrs()?
        .into_iter()
        .filter(|interface| interface.is_oper_up())
        .filter_map(|interface| match interface.ip() {
            IpAddr::V4(address) if is_lan_address(address) => Some(address),
            _ => None,
        })
        .collect();
    if addresses.is_empty() {
        return Ok(vec![SocketAddr::from((Ipv4Addr::LOCALHOST, bound.port()))]);
    }
    Ok(ordered_endpoints(
        addresses,
        bound.port(),
        route_selected_ipv4(),
    ))
}

fn route_selected_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // Connecting UDP selects the local route without sending a datagram.
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(address) if is_lan_address(address) => Some(address),
        _ => None,
    }
}

pub fn advertise_room(
    room: &str,
    bound: SocketAddr,
    player_count: u8,
) -> Result<RoomAdvertisement, DiscoveryError> {
    let addresses: Vec<_> = listen_ipv4_addresses(bound)?
        .into_iter()
        .filter(|address| matches!(address.ip(), IpAddr::V4(ip) if is_lan_address(ip)))
        .collect();
    // The DNS instance name is separate from the user-visible room TXT field.
    let token = new_resume_token()?;
    let instance = format!("strategy-{:032x}", u128::from_be_bytes(token));
    let info = room_service_info(&instance, room, &addresses, player_count)?;
    let fullname = info.get_fullname().to_owned();
    let daemon = Daemon(ServiceDaemon::new()?);
    daemon.0.register(info)?;
    Ok(RoomAdvertisement {
        daemon,
        instance,
        fullname,
        room: room.to_owned(),
        addresses,
        player_count,
    })
}

pub async fn discover_rooms(timeout: Duration) -> Result<Vec<DiscoveredRoom>, DiscoveryError> {
    let daemon = Daemon(ServiceDaemon::new()?);
    let receiver = daemon.0.browse(SERVICE_TYPE)?;
    let deadline = Instant::now() + timeout;
    let mut rooms = BTreeMap::new();

    loop {
        match timeout_at(deadline, receiver.recv_async()).await {
            Ok(Ok(ServiceEvent::ServiceResolved(service))) => {
                let name = service.get_fullname().to_owned();
                if let Some(room) = decode_room(&service) {
                    rooms.insert(name, room);
                } else {
                    rooms.remove(&name);
                }
            }
            Ok(Ok(ServiceEvent::ServiceRemoved(_, name))) => {
                rooms.remove(&name);
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) => return Err(DiscoveryError::BrowseClosed),
            Err(_) => break,
        }
    }
    daemon.0.stop_browse(SERVICE_TYPE)?;
    Ok(rooms.into_values().collect())
}

fn room_service_info(
    instance: &str,
    room: &str,
    addresses: &[SocketAddr],
    player_count: u8,
) -> Result<ServiceInfo, DiscoveryError> {
    if !valid_room(room) {
        return Err(DiscoveryError::InvalidRoom);
    }
    if !(1..=MAX_PLAYERS).contains(&player_count) {
        return Err(DiscoveryError::InvalidPlayerCount(player_count));
    }
    let port = addresses
        .first()
        .ok_or(DiscoveryError::NoIpv4Address)?
        .port();
    let ip_addresses: Vec<_> = addresses
        .iter()
        .filter(|address| {
            address.port() == port && matches!(address.ip(), IpAddr::V4(ip) if is_lan_address(ip))
        })
        .map(SocketAddr::ip)
        .collect();
    if port == 0 || ip_addresses.is_empty() {
        return Err(DiscoveryError::NoIpv4Address);
    }

    let hostname = format!("{instance}.local.");
    let properties = [
        ("room", room.to_owned()),
        ("version", PROTOCOL_VERSION.to_string()),
        ("players", player_count.to_string()),
    ];
    Ok(ServiceInfo::new(
        SERVICE_TYPE,
        instance,
        &hostname,
        ip_addresses.as_slice(),
        port,
        &properties[..],
    )?)
}

fn decode_room(service: &ResolvedService) -> Option<DiscoveredRoom> {
    let room = service.get_property_val_str("room")?;
    let version = service.get_property_val_str("version")?.parse().ok()?;
    let player_count = service.get_property_val_str("players")?.parse().ok()?;
    let port = service.get_port();
    if !valid_room(room) || !(1..=MAX_PLAYERS).contains(&player_count) || port == 0 {
        return None;
    }
    let addresses = sorted_endpoints(
        service
            .get_addresses_v4()
            .into_iter()
            .filter(|&address| is_lan_address(address)),
        port,
    );
    (!addresses.is_empty()).then_some(DiscoveredRoom {
        room: room.to_owned(),
        version,
        player_count,
        addresses,
    })
}

fn sorted_endpoints(addresses: impl IntoIterator<Item = Ipv4Addr>, port: u16) -> Vec<SocketAddr> {
    ordered_endpoints(addresses, port, None)
}

fn ordered_endpoints(
    addresses: impl IntoIterator<Item = Ipv4Addr>,
    port: u16,
    preferred: Option<Ipv4Addr>,
) -> Vec<SocketAddr> {
    let mut addresses: Vec<_> = addresses.into_iter().collect();
    addresses.sort_unstable_by_key(|&address| {
        (
            preferred != Some(address),
            address.is_link_local(),
            !address.is_private(),
            address,
        )
    });
    addresses.dedup();
    addresses
        .into_iter()
        .map(|address| SocketAddr::from((address, port)))
        .collect()
}

fn valid_room(room: &str) -> bool {
    !room.trim().is_empty() && room.len() <= MAX_ROOM_BYTES && !room.chars().any(char::is_control)
}

fn is_lan_address(address: Ipv4Addr) -> bool {
    !address.is_unspecified()
        && !address.is_loopback()
        && !address.is_multicast()
        && !address.is_broadcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_selected_address_is_preferred_without_losing_other_interfaces() {
        let virtual_adapter = Ipv4Addr::new(172, 25, 64, 1);
        let physical_lan = Ipv4Addr::new(192, 168, 10, 5);
        let addresses = [virtual_adapter, physical_lan, virtual_adapter];
        assert_eq!(
            ordered_endpoints(addresses, 31744, Some(physical_lan)),
            [
                SocketAddr::from((physical_lan, 31744)),
                SocketAddr::from((virtual_adapter, 31744)),
            ]
        );
        assert_eq!(
            ordered_endpoints(addresses, 31744, Some(Ipv4Addr::new(10, 0, 0, 1))),
            sorted_endpoints(addresses, 31744)
        );
    }

    #[test]
    fn discovery_record_uses_listener_port_and_concrete_ipv4_addresses() {
        let one = SocketAddr::from(([192, 168, 0, 12], 31744));
        let two = SocketAddr::from(([169, 254, 0, 9], 31744));
        let info = room_service_info("strategy-test", "Western Europe", &[two, one], 2).unwrap();
        assert_eq!(info.get_fullname(), "strategy-test._game._tcp.local.");
        assert_eq!(info.get_port(), 31744);
        assert_eq!(info.get_property_val_str("players"), Some("2"));
        assert_eq!(
            decode_room(&info.as_resolved_service()),
            Some(DiscoveredRoom {
                room: "Western Europe".into(),
                version: PROTOCOL_VERSION,
                player_count: 2,
                addresses: vec![one, two],
            })
        );
    }

    #[test]
    fn rejects_invalid_record_or_loopback_only_advertisement() {
        let ip = SocketAddr::from(([127, 0, 0, 1], 31744));
        assert!(matches!(
            room_service_info("strategy-test", "room", &[ip], 1),
            Err(DiscoveryError::NoIpv4Address)
        ));
        assert!(matches!(
            room_service_info("strategy-test", "room\nname", &[ip], 1),
            Err(DiscoveryError::InvalidRoom)
        ));
        assert!(matches!(
            room_service_info("strategy-test", "room", &[ip], 3),
            Err(DiscoveryError::InvalidPlayerCount(3))
        ));
        assert_eq!(listen_ipv4_addresses(ip).unwrap(), [ip]);
    }
}
