use std::{
    collections::BTreeMap,
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::Deserialize;
use strategy_data::save_game;
use strategy_net::{
    advertise_room, canonical_checksum, listen_ipv4_addresses, make_delta, new_resume_token,
    read_client, validate_hello, write_server, ClientMessage, LobbyController, LobbyCountry,
    PlayerCommand, ResumeToken, RoomAdvertisement, ServerMessage,
};
use strategy_sim::{
    Command, CommandActor, Controller, CountryId, GameState, PlayerId, SimTime, TimeCommand,
};
use tokio::{
    io::AsyncWrite,
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, watch},
    task::JoinHandle,
    time::timeout,
};

use super::metrics::{LanMetrics, Metrics, PingProbe};
use crate::{tick_host_with_log, AiDecision, AiDecisionLog};

const HOST: PlayerId = PlayerId::new(1);
const GUEST: PlayerId = PlayerId::new(2);
const CHECKSUM_INTERVAL: u64 = 30;
const FRAME_TIMEOUT: Duration = Duration::from_secs(5);
const PEER_IDLE: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiPreset {
    Balanced,
    Cautious,
    Aggressive,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostConfig {
    pub room: String,
    pub host_country_id: u32,
    pub guest_country_id: u32,
    pub ai_preset: AiPreset,
    pub grace_minutes: u16,
    pub port: u16,
}

#[derive(Clone)]
pub struct HostView {
    pub game: Arc<GameState>,
    pub sequence: u64,
    pub room: String,
    pub address: SocketAddr,
    pub guest_country_id: u32,
    pub guest_connected: bool,
    pub can_takeover: bool,
    pub temporary_ai: bool,
    pub detail: Option<String>,
}

pub struct LanHost {
    shared: Arc<Shared>,
    listener: JoinHandle<()>,
}

struct Shared {
    config: HostConfig,
    address: SocketAddr,
    state: Mutex<RoomState>,
    metrics: Mutex<Metrics>,
    updates: broadcast::Sender<Arc<ServerMessage>>,
    view: watch::Sender<HostView>,
    shutdown: watch::Sender<bool>,
}

struct RoomState {
    game: GameState,
    sequence: u64,
    canonical: Vec<u8>,
    ai_log: AiDecisionLog,
    last_checksum_tick: u64,
    guest: GuestSlot,
    advertisement: Option<RoomAdvertisement>,
    detail: Option<String>,
}

#[derive(Default)]
struct GuestSlot {
    token: Option<ResumeToken>,
    confirmed: bool,
    active: bool,
    last_command: u64,
    disconnected_at: Option<Instant>,
    temporary_ai: bool,
}

impl LanHost {
    pub async fn start(config: HostConfig, scenario: GameState) -> Result<Self, String> {
        if !valid_name(&config.room) || config.grace_minutes > 60 {
            return Err("Enter a room name and a grace period from 0 to 60 minutes.".into());
        }
        let game = configure_game(&config, scenario)?;
        let bind = if config.port == 0 {
            Ipv4Addr::LOCALHOST
        } else {
            Ipv4Addr::UNSPECIFIED
        };
        let listener = TcpListener::bind((bind, config.port))
            .await
            .map_err(|error| format!("Could not listen for LAN players: {error}"))?;
        let bound = listener.local_addr().map_err(|error| error.to_string())?;
        let (advertisement, detail) = if config.port == 0 {
            (None, None)
        } else {
            match advertise_room(&config.room, bound, 1) {
                Ok(advertisement) => (Some(advertisement), None),
                Err(error) => (
                    None,
                    Some(format!("Discovery unavailable. Join by IP: {error}")),
                ),
            }
        };
        let address = listen_ipv4_addresses(bound)
            .ok()
            .and_then(|addresses| addresses.first().copied())
            .unwrap_or_else(|| SocketAddr::from((Ipv4Addr::LOCALHOST, bound.port())));
        let canonical = save_game(&game).map_err(|error| error.to_string())?;
        let (updates, _) = broadcast::channel(64);
        let (shutdown, _) = watch::channel(false);
        let (view, _) = watch::channel(HostView {
            game: Arc::new(game.clone()),
            sequence: 0,
            room: config.room.clone(),
            address,
            guest_country_id: config.guest_country_id,
            guest_connected: false,
            can_takeover: false,
            temporary_ai: false,
            detail: detail.clone(),
        });
        let shared = Arc::new(Shared {
            config,
            address,
            state: Mutex::new(RoomState {
                game,
                sequence: 0,
                canonical,
                ai_log: AiDecisionLog::default(),
                last_checksum_tick: 0,
                guest: GuestSlot::default(),
                advertisement,
                detail,
            }),
            metrics: Mutex::default(),
            updates,
            view,
            shutdown,
        });
        let connection_slots = Arc::new(tokio::sync::Semaphore::new(8));
        let server = Arc::clone(&shared);
        let listener = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let Ok(slot) = Arc::clone(&connection_slots).try_acquire_owned() else {
                    continue;
                };
                let server = Arc::clone(&server);
                tokio::spawn(async move {
                    let _slot = slot;
                    if let Err(error) = serve_peer(server, socket).await {
                        eprintln!("LAN peer ended: {error}");
                    }
                });
            }
        });
        Ok(Self { shared, listener })
    }

    pub fn metrics(&self) -> LanMetrics {
        self.shared.metrics().snapshot()
    }

    pub fn address(&self) -> SocketAddr {
        self.shared.address
    }

    pub fn view(&self) -> HostView {
        let mut view = self.shared.view.borrow().clone();
        if let Ok(state) = self.shared.state.lock() {
            view.can_takeover = self.shared.can_takeover(&state.guest);
        }
        view
    }

    pub fn subscribe(&self) -> watch::Receiver<HostView> {
        self.shared.view.subscribe()
    }

    pub fn command(&self, command: Command) -> Result<(), String> {
        if matches!(command, Command::SetController { .. }) {
            return Err("Change country assignment through the room controls.".into());
        }
        let mut state = self.shared.lock()?;
        self.shared
            .apply(&mut state, CommandActor::Host(HOST), command)
    }

    pub fn tick(&self) -> Result<(), String> {
        let mut state = self.shared.lock()?;
        let RoomState { game, ai_log, .. } = &mut *state;
        let report = tick_host_with_log(game, ai_log).map_err(|error| format!("{error:?}"))?;
        if !report.ai_rejections.is_empty()
            || !report.scheduled_failures.is_empty()
            || !report.polity_failures.is_empty()
            || !report.military_failures.is_empty()
        {
            eprintln!(
                "LAN simulation reported a failure at tick {}: {report:?}",
                game.time().tick().get()
            );
        }
        if !report.delta.changes().is_empty() {
            self.shared.publish(&mut state)?;
        }
        Ok(())
    }

    pub fn take_over(&self) -> Result<(), String> {
        let mut state = self.shared.lock()?;
        if !self.shared.can_takeover(&state.guest) {
            return Err(
                "Temporary AI is available after the disconnected player's grace period.".into(),
            );
        }
        let country = CountryId::new(self.shared.config.guest_country_id);
        let profile = state
            .game
            .country_polity(country)
            .ok_or("The guest country is unavailable.")?
            .ai_profile
            .clone();
        self.shared.apply(
            &mut state,
            CommandActor::Host(HOST),
            Command::SetController {
                country,
                controller: Controller::Ai(profile),
            },
        )?;
        state.guest.temporary_ai = true;
        self.shared.refresh(&state);
        Ok(())
    }

    pub fn ai_log(&self) -> Vec<AiDecision> {
        self.shared
            .state
            .lock()
            .map(|state| state.ai_log.entries().cloned().collect())
            .unwrap_or_default()
    }
}

impl Drop for LanHost {
    fn drop(&mut self) {
        self.shared.shutdown.send_replace(true);
        self.listener.abort();
    }
}

impl Shared {
    fn metrics(&self) -> std::sync::MutexGuard<'_, Metrics> {
        self.metrics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    async fn send_update(
        &self,
        writer: &mut (impl AsyncWrite + Unpin),
        message: &ServerMessage,
    ) -> Result<(), String> {
        send(writer, message).await?;
        match message {
            ServerMessage::Checksum { tick, hash, .. } => {
                self.metrics().record_checksum(*tick, *hash, true)
            }
            ServerMessage::Snapshot { .. } => self.metrics().record_resync(),
            _ => {}
        }
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, RoomState>, String> {
        self.state
            .lock()
            .map_err(|_| "The LAN room is unavailable.".into())
    }

    fn can_takeover(&self, guest: &GuestSlot) -> bool {
        !guest.active
            && !guest.temporary_ai
            && guest.disconnected_at.is_some_and(|since| {
                since.elapsed() >= Duration::from_secs(u64::from(self.config.grace_minutes) * 60)
            })
    }

    fn refresh(&self, state: &RoomState) {
        self.view.send_replace(HostView {
            game: Arc::new(state.game.clone()),
            sequence: state.sequence,
            room: self.config.room.clone(),
            address: self.address,
            guest_country_id: self.config.guest_country_id,
            guest_connected: state.guest.active,
            can_takeover: self.can_takeover(&state.guest),
            temporary_ai: state.guest.temporary_ai,
            detail: state.detail.clone(),
        });
    }

    fn apply(
        &self,
        state: &mut RoomState,
        actor: CommandActor,
        command: Command,
    ) -> Result<(), String> {
        let delta = state
            .game
            .apply(actor, &command)
            .map_err(|error| format!("Command rejected ({error:?})."))?;
        if !delta.changes().is_empty() {
            self.publish(state)?;
        }
        Ok(())
    }

    fn publish(&self, state: &mut RoomState) -> Result<(), String> {
        let canonical = save_game(&state.game).map_err(|error| error.to_string())?;
        let sequence = state
            .sequence
            .checked_add(1)
            .ok_or("LAN update sequence exhausted")?;
        let tick = state.game.time().tick().get();
        let message = match make_delta(state.sequence, &state.canonical, &canonical) {
            Ok(changes) => ServerMessage::Delta { tick, changes },
            Err(_) => ServerMessage::Snapshot {
                sequence,
                tick,
                state: canonical.clone(),
            },
        };
        state.sequence = sequence;
        state.canonical = canonical;
        let _ = self.updates.send(Arc::new(message));
        if tick != state.last_checksum_tick && tick % CHECKSUM_INTERVAL == 0 {
            state.last_checksum_tick = tick;
            let _ = self.updates.send(Arc::new(ServerMessage::Checksum {
                sequence,
                tick,
                hash: canonical_checksum(&state.canonical),
            }));
        }
        self.refresh(state);
        Ok(())
    }

    fn accept(
        self: &Arc<Self>,
        version: u16,
        name: &str,
        map_version: u32,
        map_hash: [u8; 32],
        resume: Option<ResumeToken>,
    ) -> Result<PeerStart, String> {
        let mut state = self.lock()?;
        if *self.shutdown.borrow() {
            return Err("The room has closed.".into());
        }
        validate_hello(version, map_version, map_hash, state.game.map_identity())
            .map_err(|error| error.to_string())?;
        if !valid_name(name) {
            return Err("Enter a player name of 1–48 characters.".into());
        }
        if state.guest.active {
            return Err("The room already has a connected guest.".into());
        }
        let resume = match (state.guest.token, resume) {
            (None, None) => new_resume_token().map_err(|error| error.to_string())?,
            (None, Some(token)) => token,
            (Some(expected), Some(token)) if expected == token => token,
            _ => return Err("The guest seat is reserved for the original player.".into()),
        };
        if state.guest.temporary_ai {
            let country = CountryId::new(self.config.guest_country_id);
            self.apply(
                &mut state,
                CommandActor::Host(HOST),
                Command::SetController {
                    country,
                    controller: Controller::Human(GUEST),
                },
            )?;
        }
        state.guest.token = Some(resume);
        state.guest.active = true;
        state.guest.temporary_ai = false;
        state.guest.disconnected_at = None;
        set_advertised_count(&mut state, 2);
        self.refresh(&state);
        Ok(PeerStart {
            guard: PeerGuard(Arc::clone(self)),
            resume,
            lobby: self.lobby(&state),
            snapshot: snapshot(&state),
            updates: self.updates.subscribe(),
        })
    }

    fn lobby(&self, state: &RoomState) -> ServerMessage {
        let polity = state.game.polity().expect("room has playable countries");
        let guest = CountryId::new(self.config.guest_country_id);
        ServerMessage::Lobby {
            room: self.config.room.clone(),
            guest_tag: polity.countries[&guest].tag.clone(),
            countries: state
                .game
                .countries()
                .map(|(id, country)| {
                    let nation = &polity.countries[&id];
                    let controller = match country.controller() {
                        Controller::Human(player) => LobbyController::Human {
                            player_id: player.get(),
                        },
                        Controller::Ai(profile) => LobbyController::Ai {
                            profile: profile.as_str().into(),
                        },
                    };
                    LobbyCountry {
                        id: id.get(),
                        tag: nation.tag.clone(),
                        name: nation.name.clone(),
                        controller,
                    }
                })
                .collect(),
        }
    }

    fn player_command(&self, seq: u64, command: PlayerCommand) -> ServerMessage {
        let outcome = self.lock().and_then(|mut state| {
            if seq <= state.guest.last_command {
                return Err("Command sequence was already processed.".into());
            }
            state.guest.last_command = seq;
            let command = Command::try_from(command).map_err(|error| format!("{error:?}"))?;
            self.apply(&mut state, CommandActor::Guest(GUEST), command)
        });
        match outcome {
            Ok(()) => ServerMessage::CommandAccepted { seq },
            Err(reason) => ServerMessage::CommandRejected { seq, reason },
        }
    }
}

struct PeerStart {
    guard: PeerGuard,
    resume: ResumeToken,
    lobby: ServerMessage,
    snapshot: ServerMessage,
    updates: broadcast::Receiver<Arc<ServerMessage>>,
}

struct PeerGuard(Arc<Shared>);

impl Drop for PeerGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            state.guest.active = false;
            state.guest.disconnected_at = Some(Instant::now());
            if !state.guest.confirmed {
                state.guest.token = None;
                state.guest.last_command = 0;
                state.guest.disconnected_at = None;
            }
            set_advertised_count(&mut state, 1);
            self.0.refresh(&state);
        }
    }
}

fn set_advertised_count(state: &mut RoomState, count: u8) {
    if let Some(advertisement) = &mut state.advertisement {
        if let Err(error) = advertisement.set_player_count(count) {
            state.detail = Some(format!("Could not refresh discovery: {error}"));
        }
    }
}

fn snapshot(state: &RoomState) -> ServerMessage {
    ServerMessage::Snapshot {
        sequence: state.sequence,
        tick: state.game.time().tick().get(),
        state: state.canonical.clone(),
    }
}

struct PeerReader(JoinHandle<()>);

impl Drop for PeerReader {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn send(
    writer: &mut (impl AsyncWrite + Unpin),
    message: &ServerMessage,
) -> Result<(), String> {
    timeout(FRAME_TIMEOUT, write_server(writer, message))
        .await
        .map_err(|_| "LAN write timed out".to_owned())?
        .map_err(|error| error.to_string())
}

async fn serve_peer(shared: Arc<Shared>, mut socket: TcpStream) -> Result<(), String> {
    socket
        .set_nodelay(true)
        .map_err(|error| error.to_string())?;
    let hello = timeout(FRAME_TIMEOUT, read_client(&mut socket))
        .await
        .map_err(|_| "LAN handshake timed out".to_owned())?
        .map_err(|error| error.to_string())?;
    let Some(ClientMessage::Hello {
        version,
        name,
        map_version,
        map_hash,
        resume,
    }) = hello
    else {
        return send(
            &mut socket,
            &ServerMessage::Rejected {
                reason: "Hello required.".into(),
            },
        )
        .await;
    };
    let PeerStart {
        guard: _guest,
        resume,
        lobby,
        snapshot: initial,
        mut updates,
    } = match shared.accept(version, &name, map_version, map_hash, resume) {
        Ok(peer) => peer,
        Err(reason) => return send(&mut socket, &ServerMessage::Rejected { reason }).await,
    };
    let (mut reader, mut writer) = socket.into_split();
    send(
        &mut writer,
        &ServerMessage::Welcome {
            player_id: GUEST.get(),
            resume,
        },
    )
    .await?;
    send(&mut writer, &lobby).await?;
    send(&mut writer, &initial).await?;
    let mut sent = sequence_of(&initial).expect("snapshot sequence");
    let mut shutdown = shared.shutdown.subscribe();
    let mut connection_confirmed = false;
    let mut ping = PingProbe::default();
    let (inbox, mut messages) = mpsc::channel(16);
    let _reader = PeerReader(tokio::spawn(async move {
        while let Ok(Ok(Some(message))) = timeout(PEER_IDLE, read_client(&mut reader)).await {
            if inbox.send(message).await.is_err() {
                return;
            }
        }
    }));
    while !*shutdown.borrow() {
        let message = tokio::select! {
            _ = shutdown.changed() => break,
            incoming = messages.recv() => {
                if !connection_confirmed && !matches!(incoming, Some(ClientMessage::Confirm { .. })) {
                    return Err("Confirm the handshake before sending player messages.".into());
                }
                match incoming {
                Some(ClientMessage::Command { seq, command }) => shared.player_command(seq, command),
                Some(ClientMessage::PauseRequest { seq, paused }) =>
                    shared.player_command(seq, PlayerCommand::SetPaused { paused }),
                Some(ClientMessage::Ping { nonce }) => {
                    send(&mut writer, &ServerMessage::Pong { nonce }).await?;
                    ping.start(nonce);
                    send(&mut writer, &ServerMessage::Ping { nonce }).await?;
                    continue;
                }
                Some(ClientMessage::Pong { nonce }) => {
                    if let Some(elapsed) = ping.complete(nonce) {
                        shared.metrics().record_ping(elapsed);
                    }
                    continue;
                }
                Some(ClientMessage::Confirm { resume }) => {
                    let mut state = shared.lock()?;
                    if state.guest.token != Some(resume) {
                        return Err("The handshake confirmation token is invalid.".into());
                    }
                    state.guest.confirmed = true;
                    connection_confirmed = true;
                    continue;
                }
                Some(ClientMessage::Resync) => snapshot(&*shared.lock()?),
                Some(ClientMessage::ClaimCountry { tag }) => {
                    let state = shared.lock()?;
                    let country = CountryId::new(shared.config.guest_country_id);
                    if state.game.country_polity(country).is_some_and(|nation| nation.tag == tag) {
                        shared.lobby(&state)
                    } else {
                        ServerMessage::Rejected { reason: "The host assigned a different guest country.".into() }
                    }
                }
                Some(ClientMessage::SetController { .. }) => ServerMessage::Rejected {
                    reason: "Only the host can change controllers.".into(),
                },
                Some(ClientMessage::Hello { .. }) | None => break,
                }
            },
            outgoing = updates.recv() => match outgoing {
                Ok(message) => {
                    match message.as_ref() {
                        ServerMessage::Checksum { sequence, .. } if *sequence != sent => continue,
                        ServerMessage::Checksum { .. } => {},
                        _ => {
                            let sequence = sequence_of(&message).expect("sequenced broadcast");
                            if sequence <= sent {
                                continue;
                            }
                            sent = sequence;
                        }
                    }
                    shared.send_update(&mut writer, &message).await?;
                    continue;
                }
                Err(broadcast::error::RecvError::Lagged(_)) => snapshot(&*shared.lock()?),
                Err(broadcast::error::RecvError::Closed) => break,
            }
        };
        if let Some(sequence) = sequence_of(&message) {
            sent = sequence;
        }
        shared.send_update(&mut writer, &message).await?;
    }
    Ok(())
}

fn sequence_of(message: &ServerMessage) -> Option<u64> {
    match message {
        ServerMessage::Snapshot { sequence, .. } => Some(*sequence),
        ServerMessage::Delta { changes, .. } => Some(changes.sequence),
        _ => None,
    }
}

fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().count() <= 48 && !name.chars().any(char::is_control)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetFile {
    schema_version: u32,
    presets: BTreeMap<String, Preset>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preset {
    aggression_scale: u16,
    war_min_score_adjustment: i32,
}

fn configure_game(config: &HostConfig, scenario: GameState) -> Result<GameState, String> {
    let mut polity = scenario
        .polity()
        .ok_or("The room needs a playable scenario.")?
        .clone();
    let military = scenario
        .war_state()
        .ok_or("The room needs military rules.")?;
    let host_country = CountryId::new(config.host_country_id);
    let guest_country = CountryId::new(config.guest_country_id);
    if host_country == guest_country
        || !polity.countries.contains_key(&host_country)
        || !polity.countries.contains_key(&guest_country)
    {
        return Err("Choose two different playable countries.".into());
    }
    let data: PresetFile = serde_json::from_str(include_str!("../../../data/ai/lan_presets.json"))
        .map_err(|error| format!("Invalid LAN presets: {error}"))?;
    if data.schema_version != 1 {
        return Err("Unsupported LAN preset schema.".into());
    }
    let key = match config.ai_preset {
        AiPreset::Balanced => "balanced",
        AiPreset::Cautious => "cautious",
        AiPreset::Aggressive => "aggressive",
    };
    let preset = data
        .presets
        .get(key)
        .ok_or("The AI preset is unavailable.")?;
    if preset.aggression_scale > 2000 || !(-1000..=1000).contains(&preset.war_min_score_adjustment)
    {
        return Err("The AI preset exceeds the allowed tuning range.".into());
    }
    for profile in polity.ai_profiles.values_mut() {
        let aggression = u32::from(profile.weights.aggression) * u32::from(preset.aggression_scale);
        profile.weights.aggression = (aggression / 1000).min(1000) as u16;
        profile.thresholds.war_min_score = profile
            .thresholds
            .war_min_score
            .saturating_add(preset.war_min_score_adjustment);
    }
    let assignments: Vec<_> = polity
        .countries
        .iter()
        .map(|(&country, nation)| {
            let controller = if country == host_country {
                Controller::Human(HOST)
            } else if country == guest_country {
                Controller::Human(GUEST)
            } else {
                Controller::Ai(nation.ai_profile.clone())
            };
            (country, controller)
        })
        .collect();
    let time = SimTime::new(scenario.time().start_date(), scenario.time().speed(), true);
    let mut game = GameState::new(
        time,
        scenario.seed(),
        scenario.province_graph().clone(),
        scenario
            .countries()
            .map(|(id, country)| (id, country.clone()))
            .collect(),
        [],
    )
    .map_err(|error| format!("{error:?}"))?
    .with_polity(polity)
    .map_err(|error| format!("{error:?}"))?
    .with_war_state(military.clone())
    .map_err(|error| format!("{error:?}"))?;
    for (country, controller) in assignments {
        game.apply(
            CommandActor::Host(HOST),
            &Command::SetController {
                country,
                controller,
            },
        )
        .map_err(|error| format!("{error:?}"))?;
    }
    game.apply(
        CommandActor::Host(HOST),
        &Command::Time(TimeCommand::SetPaused(true)),
    )
    .map_err(|error| format!("{error:?}"))?;
    Ok(game)
}
