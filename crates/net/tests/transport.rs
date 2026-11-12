use std::{io::ErrorKind, num::NonZeroU32, time::Duration};

use strategy_net::{
    read_client, read_server, validate_hello, write_client, write_server, ClientMessage,
    CommandDecodeError, HandshakeError, PlayerCommand, ServerMessage, MAX_CLIENT_FRAME_BYTES,
    MAX_SERVER_FRAME_BYTES, PROTOCOL_VERSION,
};
use strategy_sim::{Command, MapIdentity};
use tokio::net::{TcpListener, TcpStream};

fn identity() -> MapIdentity {
    MapIdentity {
        version: NonZeroU32::new(2).unwrap(),
        sha256: [42; 32],
    }
}

#[test]
fn handshake_rejects_each_incompatible_identity_component() {
    let map = identity();
    assert_eq!(PROTOCOL_VERSION, 11);
    assert_eq!(validate_hello(PROTOCOL_VERSION, 2, [42; 32], map), Ok(()));
    assert_eq!(
        validate_hello(3, 2, [42; 32], map),
        Err(HandshakeError::ProtocolVersion { received: 3 })
    );
    assert_eq!(
        validate_hello(PROTOCOL_VERSION, 1, [42; 32], map),
        Err(HandshakeError::MapVersion {
            expected: 2,
            received: 1
        })
    );
    assert_eq!(
        validate_hello(PROTOCOL_VERSION, 2, [0; 32], map),
        Err(HandshakeError::MapHash)
    );
}

#[tokio::test]
async fn frame_length_is_checked_before_payload_read() {
    for size in [0, (MAX_CLIENT_FRAME_BYTES + 1) as u32, u32::MAX] {
        let header = size.to_be_bytes();
        assert_eq!(
            read_client(&mut header.as_slice())
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidData
        );
    }
    let header = ((MAX_SERVER_FRAME_BYTES + 1) as u32).to_be_bytes();
    assert_eq!(
        read_server(&mut header.as_slice())
            .await
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidData
    );
}

#[tokio::test]
async fn clean_eof_is_distinct_from_truncated_or_malformed_frames() {
    assert_eq!(read_client(&mut [].as_slice()).await.unwrap(), None);
    for truncated in [vec![0], vec![0, 0, 0], vec![0, 0, 0, 2, 0]] {
        assert_eq!(
            read_client(&mut truncated.as_slice())
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::UnexpectedEof
        );
    }
    let payload = [0xff; 10];
    let mut malformed = (payload.len() as u32).to_be_bytes().to_vec();
    malformed.extend_from_slice(&payload);
    assert_eq!(
        read_client(&mut malformed.as_slice())
            .await
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidData
    );

    let mut payload = postcard::to_allocvec(&ClientMessage::Ping { nonce: 5 }).unwrap();
    payload.push(0);
    let mut trailing = (payload.len() as u32).to_be_bytes().to_vec();
    trailing.extend(payload);
    assert_eq!(
        read_client(&mut trailing.as_slice())
            .await
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidData
    );
}

#[tokio::test]
async fn oversized_outgoing_message_writes_no_partial_frame() {
    let mut output = Vec::new();
    let message = ClientMessage::ClaimCountry {
        tag: "x".repeat(MAX_CLIENT_FRAME_BYTES),
    };
    assert_eq!(
        write_client(&mut output, &message)
            .await
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidInput
    );
    assert!(output.is_empty());
}

#[test]
fn command_conversion_preserves_raw_fixed_units_and_validates_speed() {
    let commands = [
        PlayerCommand::SetFiscalPolicy {
            country: 4,
            tax_rate: 100,
            monthly_spending: i64::MAX,
        },
        PlayerCommand::NominateHeir {
            country: 4,
            heir: Some(8),
        },
        PlayerCommand::MoveArmy {
            country: 4,
            army: 3,
            destination: 99,
        },
        PlayerCommand::OfferPeace {
            country: 4,
            war: 2,
            cede_goal: true,
        },
        PlayerCommand::OfferAlliance {
            country: 4,
            toward: 9,
        },
        PlayerCommand::AcceptAlliance {
            country: 9,
            from: 4,
        },
        PlayerCommand::RejectAlliance {
            country: 9,
            from: 4,
        },
        PlayerCommand::BreakAlliance {
            country: 4,
            other: 9,
        },
        PlayerCommand::OfferRoyalMarriage {
            country: 4,
            character: 10,
            toward: 9,
            partner: 20,
        },
        PlayerCommand::AcceptRoyalMarriage {
            country: 9,
            from: 4,
            character: 10,
            partner: 20,
        },
        PlayerCommand::RejectRoyalMarriage {
            country: 9,
            from: 4,
            character: 10,
            partner: 20,
        },
        PlayerCommand::ChooseEvent {
            country: 4,
            key: "harvest_festival".into(),
            option: 1,
        },
        PlayerCommand::UnlockTechnology {
            country: 1,
            key: "recordkeeping".into(),
        },
        PlayerCommand::UnlockIdea {
            country: 1,
            key: "english_longbow".into(),
        },
        PlayerCommand::Guarantee {
            country: 4,
            protected: 9,
        },
        PlayerCommand::RevokeGuarantee {
            country: 4,
            protected: 9,
        },
        PlayerCommand::OfferVassalage {
            country: 4,
            toward: 9,
        },
        PlayerCommand::AcceptVassalage {
            country: 9,
            from: 4,
        },
        PlayerCommand::RejectVassalage {
            country: 9,
            from: 4,
        },
        PlayerCommand::ReleaseVassal {
            country: 4,
            subject: 9,
        },
        PlayerCommand::DeclareIndependence { country: 9 },
        PlayerCommand::SetPaused { paused: true },
        PlayerCommand::SetSpeed { speed: 5 },
    ];
    for wire in commands {
        let command = Command::try_from(wire.clone()).unwrap();
        assert_eq!(PlayerCommand::from(command), wire);
    }
    for speed in [0, 6, u8::MAX] {
        assert_eq!(
            Command::try_from(PlayerCommand::SetSpeed { speed }),
            Err(CommandDecodeError::InvalidSpeed(speed))
        );
    }
}

#[tokio::test]
async fn loopback_tcp_exchanges_handshake_command_and_consecutive_frames() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let endpoint = listener.local_addr().unwrap();
        let host = async {
            let (mut stream, _) = listener.accept().await.unwrap();
            let Some(ClientMessage::Hello {
                version,
                map_version,
                map_hash,
                ..
            }) = read_client(&mut stream).await.unwrap()
            else {
                panic!("expected hello")
            };
            validate_hello(version, map_version, map_hash, identity()).unwrap();
            write_server(
                &mut stream,
                &ServerMessage::Welcome {
                    player_id: 2,
                    resume: [7; 16],
                },
            )
            .await
            .unwrap();
            assert_eq!(
                read_client(&mut stream).await.unwrap(),
                Some(ClientMessage::Confirm { resume: [7; 16] })
            );
            write_server(
                &mut stream,
                &ServerMessage::Snapshot {
                    sequence: 0,
                    tick: 0,
                    state: vec![1, 2, 3],
                },
            )
            .await
            .unwrap();
            assert_eq!(
                read_client(&mut stream).await.unwrap(),
                Some(ClientMessage::PauseRequest {
                    seq: 8,
                    paused: true
                })
            );
            write_server(&mut stream, &ServerMessage::CommandAccepted { seq: 8 })
                .await
                .unwrap();
            assert_eq!(
                read_client(&mut stream).await.unwrap(),
                Some(ClientMessage::Command {
                    seq: 11,
                    command: PlayerCommand::OfferRoyalMarriage {
                        country: 1,
                        character: 10,
                        toward: 2,
                        partner: 20
                    },
                })
            );
            write_server(&mut stream, &ServerMessage::CommandAccepted { seq: 11 })
                .await
                .unwrap();
            assert_eq!(
                read_client(&mut stream).await.unwrap(),
                Some(ClientMessage::Ping { nonce: 9 })
            );
            write_server(&mut stream, &ServerMessage::Pong { nonce: 9 })
                .await
                .unwrap();
            write_server(&mut stream, &ServerMessage::Ping { nonce: 10 })
                .await
                .unwrap();
            assert_eq!(
                read_client(&mut stream).await.unwrap(),
                Some(ClientMessage::Pong { nonce: 10 })
            );
        };
        let guest = async {
            let mut stream = TcpStream::connect(endpoint).await.unwrap();
            write_client(
                &mut stream,
                &ClientMessage::Hello {
                    version: PROTOCOL_VERSION,
                    name: "Guest".into(),
                    map_version: 2,
                    map_hash: [42; 32],
                    resume: None,
                },
            )
            .await
            .unwrap();
            assert_eq!(
                read_server(&mut stream).await.unwrap(),
                Some(ServerMessage::Welcome {
                    player_id: 2,
                    resume: [7; 16]
                })
            );
            write_client(&mut stream, &ClientMessage::Confirm { resume: [7; 16] })
                .await
                .unwrap();
            assert_eq!(
                read_server(&mut stream).await.unwrap(),
                Some(ServerMessage::Snapshot {
                    sequence: 0,
                    tick: 0,
                    state: vec![1, 2, 3]
                })
            );
            write_client(
                &mut stream,
                &ClientMessage::PauseRequest {
                    seq: 8,
                    paused: true,
                },
            )
            .await
            .unwrap();
            assert_eq!(
                read_server(&mut stream).await.unwrap(),
                Some(ServerMessage::CommandAccepted { seq: 8 })
            );
            write_client(
                &mut stream,
                &ClientMessage::Command {
                    seq: 11,
                    command: PlayerCommand::OfferRoyalMarriage {
                        country: 1,
                        character: 10,
                        toward: 2,
                        partner: 20,
                    },
                },
            )
            .await
            .unwrap();
            assert_eq!(
                read_server(&mut stream).await.unwrap(),
                Some(ServerMessage::CommandAccepted { seq: 11 })
            );
            write_client(&mut stream, &ClientMessage::Ping { nonce: 9 })
                .await
                .unwrap();
            assert_eq!(
                read_server(&mut stream).await.unwrap(),
                Some(ServerMessage::Pong { nonce: 9 })
            );
            assert_eq!(
                read_server(&mut stream).await.unwrap(),
                Some(ServerMessage::Ping { nonce: 10 })
            );
            write_client(&mut stream, &ClientMessage::Pong { nonce: 10 })
                .await
                .unwrap();
            assert_eq!(read_server(&mut stream).await.unwrap(), None);
        };
        tokio::join!(host, guest);
    })
    .await
    .expect("loopback handshake and framed exchange complete promptly");
}
