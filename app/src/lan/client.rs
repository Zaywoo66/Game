use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};

use strategy_data::load_game;
use strategy_net::{
    canonical_checksum, read_server, write_client, ClientMessage, PlayerCommand, Replica,
    ServerMessage, PROTOCOL_VERSION,
};
use strategy_sim::{Command, GameState, ProvinceGraph, TimeCommand};
use tokio::{
    net::{tcp::OwnedWriteHalf, TcpStream},
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
    time::{interval, sleep, timeout},
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const RECONNECT_DELAY: Duration = Duration::from_secs(1);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_PENDING: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientPhase {
    Connected,
    Reconnecting,
    Resyncing,
}

#[derive(Clone)]
pub struct ClientView {
    pub game: Arc<GameState>,
    pub sequence: u64,
    /// Increases whenever a full snapshot replaces the byte stream, including
    /// a same-sequence correction that a watch receiver may otherwise coalesce.
    pub revision: u64,
    pub phase: ClientPhase,
    pub room: String,
    pub address: SocketAddr,
    pub guest_tag: String,
    pub verified_checksums: u64,
    pub checksum_mismatches: u64,
    pub detail: Option<String>,
}

pub struct LanClient {
    view: watch::Receiver<ClientView>,
    commands: mpsc::Sender<Request>,
    task: JoinHandle<()>,
}

impl LanClient {
    pub async fn join(
        address: SocketAddr,
        name: String,
        graph: ProvinceGraph,
    ) -> Result<Self, String> {
        let session = connect(address, &name, &graph, None).await?;
        let (view_tx, view) = watch::channel(ClientView {
            game: Arc::clone(&session.game),
            sequence: session.replica.sequence(),
            revision: 0,
            phase: ClientPhase::Connected,
            room: session.room.clone(),
            address,
            guest_tag: session.guest_tag.clone(),
            verified_checksums: 0,
            checksum_mismatches: 0,
            detail: None,
        });
        let (commands, requests) = mpsc::channel(MAX_PENDING);
        let task = tokio::spawn(run_client(address, name, graph, session, view_tx, requests));
        Ok(Self {
            view,
            commands,
            task,
        })
    }

    pub fn subscribe(&self) -> watch::Receiver<ClientView> {
        self.view.clone()
    }

    pub async fn command(&self, command: Command) -> Result<(), String> {
        let (reply, result) = oneshot::channel();
        {
            let view = self.view.borrow();
            if view.phase != ClientPhase::Connected {
                return Err("LAN session is not synchronized".into());
            }
            self.commands
                .try_send(Request { command, reply })
                .map_err(|error| match error {
                    mpsc::error::TrySendError::Full(_) => "Too many LAN commands pending",
                    mpsc::error::TrySendError::Closed(_) => "LAN connection closed",
                })?;
        }
        timeout(REQUEST_TIMEOUT, result)
            .await
            .map_err(|_| "Timed out waiting for the host command result")?
            .map_err(|_| "LAN connection closed before the command result")?
    }
}

impl Drop for LanClient {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Request {
    command: Command,
    reply: oneshot::Sender<Result<(), String>>,
}

struct Session {
    socket: Socket,
    resume: [u8; 16],
    room: String,
    guest_tag: String,
    replica: Replica,
    game: Arc<GameState>,
}

struct Socket {
    writer: OwnedWriteHalf,
    incoming: mpsc::Receiver<Result<ServerMessage, String>>,
    reader: JoinHandle<()>,
}

impl Socket {
    fn new(stream: TcpStream) -> Self {
        let (mut reader, writer) = stream.into_split();
        let (messages, incoming) = mpsc::channel(8);
        // Keep the frame read in its own task so selecting over writes, timeouts,
        // and incoming messages cannot cancel a partially read frame.
        let reader = tokio::spawn(async move {
            loop {
                let message = match timeout(READ_TIMEOUT, read_server(&mut reader)).await {
                    Ok(Ok(Some(message))) => Ok(message),
                    Ok(Ok(None)) => Err("Host closed the LAN connection".into()),
                    Ok(Err(error)) => Err(error.to_string()),
                    Err(_) => Err("Timed out waiting for the LAN host".into()),
                };
                let finished = message.is_err();
                if messages.send(message).await.is_err() || finished {
                    break;
                }
            }
        });
        Self {
            writer,
            incoming,
            reader,
        }
    }

    async fn send(&mut self, message: &ClientMessage) -> Result<(), String> {
        timeout(REQUEST_TIMEOUT, write_client(&mut self.writer, message))
            .await
            .map_err(|_| "Timed out writing to the LAN host")?
            .map_err(|error| error.to_string())
    }

    async fn receive(&mut self) -> Result<ServerMessage, String> {
        self.incoming.recv().await.ok_or("LAN connection closed")?
    }

    async fn handshake_message(&mut self) -> Result<ServerMessage, String> {
        match timeout(REQUEST_TIMEOUT, self.receive())
            .await
            .map_err(|_| "Timed out waiting for the LAN handshake")??
        {
            ServerMessage::Rejected { reason } => Err(reason),
            message => Ok(message),
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

async fn connect(
    address: SocketAddr,
    name: &str,
    graph: &ProvinceGraph,
    resume: Option<[u8; 16]>,
) -> Result<Session, String> {
    let stream = timeout(REQUEST_TIMEOUT, TcpStream::connect(address))
        .await
        .map_err(|_| "Timed out connecting to the LAN host")?
        .map_err(|error| error.to_string())?;
    stream
        .set_nodelay(true)
        .map_err(|error| error.to_string())?;
    let mut socket = Socket::new(stream);
    let map = graph.identity();
    socket
        .send(&ClientMessage::Hello {
            version: PROTOCOL_VERSION,
            name: name.to_owned(),
            map_version: map.version.get(),
            map_hash: map.sha256,
            resume,
        })
        .await?;
    let ServerMessage::Welcome { resume, .. } = socket.handshake_message().await? else {
        return Err("Host did not send Welcome".into());
    };
    let ServerMessage::Lobby {
        room, guest_tag, ..
    } = socket.handshake_message().await?
    else {
        return Err("Host did not send Lobby".into());
    };
    let ServerMessage::Snapshot {
        sequence,
        tick,
        state,
    } = socket.handshake_message().await?
    else {
        return Err("Host did not send an initial Snapshot".into());
    };
    let replica = Replica::new(sequence, state).map_err(|error| error.to_string())?;
    let game = read_game(&replica, tick, graph)?;
    socket.send(&ClientMessage::Confirm { resume }).await?;
    Ok(Session {
        socket,
        resume,
        room,
        guest_tag,
        replica,
        game,
    })
}

fn read_game(
    replica: &Replica,
    tick: u64,
    graph: &ProvinceGraph,
) -> Result<Arc<GameState>, String> {
    let game = load_game(replica.bytes(), graph.clone()).map_err(|error| error.to_string())?;
    if game.time().tick().get() != tick {
        return Err("LAN state tick does not match the message".into());
    }
    Ok(Arc::new(game))
}

async fn run_client(
    address: SocketAddr,
    name: String,
    graph: ProvinceGraph,
    mut session: Session,
    view: watch::Sender<ClientView>,
    mut requests: mpsc::Receiver<Request>,
) {
    let mut last_command = 0u64;
    loop {
        let error = match run_session(
            &mut session,
            &graph,
            &view,
            &mut requests,
            &mut last_command,
        )
        .await
        {
            Ok(()) => return,
            Err(error) => error,
        };
        let resume = session.resume;
        drop(session);
        set_phase(&view, ClientPhase::Reconnecting, Some(error));
        while let Ok(request) = requests.try_recv() {
            let _ = request.reply.send(Err("LAN connection interrupted".into()));
        }
        session = loop {
            sleep(RECONNECT_DELAY).await;
            match connect(address, &name, &graph, Some(resume)).await {
                Ok(connected) if connected.replica.sequence() >= view.borrow().sequence => {
                    break connected;
                }
                Ok(_) => set_phase(
                    &view,
                    ClientPhase::Reconnecting,
                    Some("Host sent an out-of-date LAN snapshot".into()),
                ),
                Err(error) => set_phase(&view, ClientPhase::Reconnecting, Some(error)),
            }
        };
        publish_snapshot(&view, &session, true);
    }
}

async fn run_session(
    session: &mut Session,
    graph: &ProvinceGraph,
    view: &watch::Sender<ClientView>,
    requests: &mut mpsc::Receiver<Request>,
    last_command: &mut u64,
) -> Result<(), String> {
    let mut pending = BTreeMap::new();
    let mut heartbeat = interval(HEARTBEAT_INTERVAL);
    let mut nonce = 0u64;
    let mut resync_deadline = None;
    loop {
        tokio::select! {
            request = requests.recv() => {
                let Some(request) = request else { return Ok(()); };
                if request.reply.is_closed() {
                    continue;
                }
                if resync_deadline.is_some() {
                    let _ = request.reply.send(Err("LAN session is resynchronizing".into()));
                    continue;
                }
                pending.retain(|_, reply: &mut oneshot::Sender<Result<(), String>>| !reply.is_closed());
                if pending.len() >= MAX_PENDING {
                    let _ = request.reply.send(Err("Too many LAN commands pending".into()));
                    continue;
                }
                let seq = last_command.checked_add(1).ok_or("LAN command sequence exhausted")?;
                let message = match &request.command {
                    Command::Time(TimeCommand::SetPaused(paused)) => ClientMessage::PauseRequest { seq, paused: *paused },
                    command => ClientMessage::Command { seq, command: PlayerCommand::from(command) },
                };
                *last_command = seq;
                session.socket.send(&message).await?;
                pending.insert(seq, request.reply);
            }
            received = session.socket.receive() => {
                match received? {
                    ServerMessage::Delta { tick, changes } if resync_deadline.is_none() => {
                        let result = session.replica.apply(&changes).map_err(|error| error.to_string())
                            .and_then(|()| read_game(&session.replica, tick, graph));
                        match result {
                            Ok(game) => {
                                session.game = game;
                                publish_snapshot(view, session, false);
                            }
                            Err(error) => {
                                begin_resync(session, view, error).await?;
                                resync_deadline = Some(tokio::time::Instant::now() + REQUEST_TIMEOUT);
                            }
                        }
                    }
                    ServerMessage::Snapshot { sequence, tick, state } => {
                        if sequence < view.borrow().sequence {
                            return Err("Host sent an out-of-date LAN snapshot".into());
                        }
                        session.replica = Replica::new(sequence, state).map_err(|error| error.to_string())?;
                        session.game = read_game(&session.replica, tick, graph)?;
                        publish_snapshot(view, session, true);
                        resync_deadline = None;
                    }
                    ServerMessage::Checksum { sequence, tick, hash } if resync_deadline.is_none() => {
                        let local = canonical_checksum(session.replica.bytes());
                        if sequence == session.replica.sequence() && tick == session.game.time().tick().get() && hash == local {
                            view.send_modify(|view| view.verified_checksums += 1);
                        } else {
                            let error = format!("LAN checksum mismatch at host sequence {sequence}, tick {tick}: host {hash:02x?}, client {local:02x?} at sequence {}", session.replica.sequence());
                            eprintln!("{error}");
                            view.send_modify(|view| view.checksum_mismatches += 1);
                            begin_resync(session, view, error).await?;
                            resync_deadline = Some(tokio::time::Instant::now() + REQUEST_TIMEOUT);
                        }
                    }
                    ServerMessage::CommandAccepted { seq } => {
                        if let Some(reply) = pending.remove(&seq) {
                            let _ = reply.send(Ok(()));
                        }
                    }
                    ServerMessage::CommandRejected { seq, reason } => {
                        if let Some(reply) = pending.remove(&seq) {
                            let _ = reply.send(Err(reason));
                        }
                    }
                    ServerMessage::Lobby { room, guest_tag, .. } => {
                        session.room = room;
                        session.guest_tag = guest_tag;
                        view.send_modify(|view| {
                            view.room.clone_from(&session.room);
                            view.guest_tag.clone_from(&session.guest_tag);
                        });
                    }
                    ServerMessage::Rejected { reason } => return Err(reason),
                    ServerMessage::Delta { .. } | ServerMessage::Checksum { .. }
                    | ServerMessage::Pause { .. } | ServerMessage::Speed { .. } | ServerMessage::Pong { .. } => {}
                    ServerMessage::Welcome { .. } => return Err("Unexpected LAN Welcome".into()),
                }
            }
            _ = heartbeat.tick() => {
                session.socket.send(&ClientMessage::Ping { nonce }).await?;
                nonce = nonce.wrapping_add(1);
            }
            _ = async {
                if let Some(deadline) = resync_deadline {
                    tokio::time::sleep_until(deadline).await;
                }
            }, if resync_deadline.is_some() => return Err("Timed out waiting for a LAN resync snapshot".into()),
        }
    }
}

async fn begin_resync(
    session: &mut Session,
    view: &watch::Sender<ClientView>,
    error: String,
) -> Result<(), String> {
    set_phase(view, ClientPhase::Resyncing, Some(error));
    session.socket.send(&ClientMessage::Resync).await
}

fn set_phase(view: &watch::Sender<ClientView>, phase: ClientPhase, detail: Option<String>) {
    view.send_modify(|view| {
        view.phase = phase;
        view.detail = detail;
    });
}

fn publish_snapshot(view: &watch::Sender<ClientView>, session: &Session, full_snapshot: bool) {
    view.send_modify(|view| {
        if full_snapshot {
            view.revision = view.revision.saturating_add(1);
        }
        view.game = Arc::clone(&session.game);
        view.sequence = session.replica.sequence();
        view.phase = ClientPhase::Connected;
        view.room.clone_from(&session.room);
        view.guest_tag.clone_from(&session.guest_tag);
        view.detail = None;
    });
}
