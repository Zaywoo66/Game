use std::time::{Duration, Instant};

use strategy_app::{
    lan::{AiPreset, ClientPhase, HostConfig, LanClient, LanHost},
    load_embedded_scenario, tick_host_with_log, AiDecisionLog,
};
use strategy_data::{load_game, save_game};
use strategy_net::{
    canonical_checksum, read_client, read_server, write_client, write_server, ClientMessage,
    LobbyController, PlayerCommand, ServerMessage, PROTOCOL_VERSION,
};
use strategy_sim::{
    Command, CommandActor, Controller, CountryId, Date, GameState, Opinion, PlayerId, Speed,
    TimeCommand,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
    time::timeout,
};

const WAIT: Duration = Duration::from_secs(15);

#[tokio::test]
async fn lan_players_can_reject_accept_and_break_alliance_proposals() {
    let scenario = load_embedded_scenario().unwrap();
    let graph = scenario.province_graph().clone();
    let host_id = CountryId::new(country(&scenario, "por"));
    let guest_id = CountryId::new(country(&scenario, "sco"));
    let host = LanHost::start(
        HostConfig {
            room: "Alliance commands".into(),
            host_country_id: host_id.get(),
            guest_country_id: guest_id.get(),
            ai_preset: AiPreset::Balanced,
            grace_minutes: 0,
            port: 0,
        },
        scenario,
    )
    .await
    .unwrap();
    let guest = LanClient::join(host.address(), "Alliance guest".into(), graph)
        .await
        .unwrap();
    let mut view = guest.subscribe();

    host.command(Command::OfferAlliance {
        country: host_id,
        toward: guest_id,
    })
    .unwrap();
    timeout(WAIT, async {
        loop {
            if view
                .borrow()
                .game
                .diplomacy()
                .and_then(|d| d.offer(host_id, guest_id))
                .is_some()
            {
                break;
            }
            view.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    guest
        .command(Command::RejectAlliance {
            country: guest_id,
            from: host_id,
        })
        .await
        .unwrap();
    assert!(host
        .view()
        .game
        .diplomacy()
        .unwrap()
        .offer(host_id, guest_id)
        .is_none());
    host.command(Command::OfferAlliance {
        country: host_id,
        toward: guest_id,
    })
    .unwrap();
    guest
        .command(Command::AcceptAlliance {
            country: guest_id,
            from: host_id,
        })
        .await
        .unwrap();
    assert!(host.view().game.allied(host_id, guest_id));
    timeout(WAIT, async {
        loop {
            if view.borrow().game.allied(host_id, guest_id) {
                break;
            }
            view.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    guest
        .command(Command::BreakAlliance {
            country: guest_id,
            other: host_id,
        })
        .await
        .unwrap();
    assert!(!host.view().game.allied(host_id, guest_id));
    assert_eq!(
        host.view()
            .game
            .opinion(host_id, guest_id)
            .unwrap()
            .points(),
        -25
    );
    timeout(WAIT, async {
        loop {
            if !view.borrow().game.allied(host_id, guest_id)
                && view
                    .borrow()
                    .game
                    .opinion(host_id, guest_id)
                    .is_some_and(|o| o.points() == -25)
            {
                break;
            }
            view.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn lan_players_can_reject_accept_and_release_vassalage() {
    let scenario = load_embedded_scenario().unwrap();
    let graph = scenario.province_graph().clone();
    let host_id = CountryId::new(country(&scenario, "por"));
    let guest_id = CountryId::new(country(&scenario, "sco"));
    let host = LanHost::start(
        HostConfig {
            room: "Vassalage commands".into(),
            host_country_id: host_id.get(),
            guest_country_id: guest_id.get(),
            ai_preset: AiPreset::Balanced,
            grace_minutes: 0,
            port: 0,
        },
        scenario,
    )
    .await
    .unwrap();
    let guest = LanClient::join(host.address(), "Vassalage guest".into(), graph)
        .await
        .unwrap();
    let mut view = guest.subscribe();
    host.command(Command::OfferVassalage {
        country: host_id,
        toward: guest_id,
    })
    .unwrap();
    timeout(WAIT, async {
        loop {
            if view
                .borrow()
                .game
                .diplomacy()
                .is_some_and(|d| d.vassal_offers.contains_key(&(host_id, guest_id)))
            {
                break;
            }
            view.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    guest
        .command(Command::RejectVassalage {
            country: guest_id,
            from: host_id,
        })
        .await
        .unwrap();
    assert!(!host
        .view()
        .game
        .diplomacy()
        .unwrap()
        .vassal_offers
        .contains_key(&(host_id, guest_id)));
    host.command(Command::OfferVassalage {
        country: host_id,
        toward: guest_id,
    })
    .unwrap();
    guest
        .command(Command::AcceptVassalage {
            country: guest_id,
            from: host_id,
        })
        .await
        .unwrap();
    assert_eq!(
        host.view().game.diplomacy().unwrap().overlord_of(guest_id),
        Some(host_id)
    );
    timeout(WAIT, async {
        loop {
            if view
                .borrow()
                .game
                .diplomacy()
                .and_then(|d| d.overlord_of(guest_id))
                == Some(host_id)
            {
                break;
            }
            view.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    host.command(Command::ReleaseVassal {
        country: host_id,
        subject: guest_id,
    })
    .unwrap();
    timeout(WAIT, async {
        loop {
            if view
                .borrow()
                .game
                .diplomacy()
                .is_some_and(|d| d.overlord_of(guest_id).is_none())
            {
                break;
            }
            view.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn lan_guest_guarantee_and_revoke_replicate_to_host_and_guest() {
    let scenario = load_embedded_scenario().unwrap();
    let graph = scenario.province_graph().clone();
    let host_id = CountryId::new(country(&scenario, "por"));
    let guest_id = CountryId::new(country(&scenario, "sco"));
    let host = LanHost::start(
        HostConfig {
            room: "Guarantee commands".into(),
            host_country_id: host_id.get(),
            guest_country_id: guest_id.get(),
            ai_preset: AiPreset::Balanced,
            grace_minutes: 0,
            port: 0,
        },
        scenario,
    )
    .await
    .unwrap();
    let guest = LanClient::join(host.address(), "Guarantee guest".into(), graph)
        .await
        .unwrap();
    let mut view = guest.subscribe();
    guest
        .command(Command::Guarantee {
            country: guest_id,
            protected: host_id,
        })
        .await
        .unwrap();
    assert!(host
        .view()
        .game
        .diplomacy()
        .unwrap()
        .guarantees(guest_id, host_id));
    timeout(WAIT, async {
        loop {
            if view
                .borrow()
                .game
                .diplomacy()
                .is_some_and(|d| d.guarantees(guest_id, host_id))
            {
                break;
            }
            view.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    guest
        .command(Command::RevokeGuarantee {
            country: guest_id,
            protected: host_id,
        })
        .await
        .unwrap();
    assert!(!host
        .view()
        .game
        .diplomacy()
        .unwrap()
        .guarantees(guest_id, host_id));
    assert_eq!(
        host.view()
            .game
            .opinion(host_id, guest_id)
            .unwrap()
            .points(),
        -10
    );
    timeout(WAIT, async {
        loop {
            let updated = {
                let current = view.borrow();
                let game = &current.game;
                game.diplomacy()
                    .is_some_and(|d| !d.guarantees(guest_id, host_id))
                    && game
                        .opinion(host_id, guest_id)
                        .is_some_and(|o| o.points() == -10)
            };
            if updated {
                break;
            }
            view.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

fn country(game: &GameState, tag: &str) -> u32 {
    let countries = &game.polity().unwrap().countries;
    countries
        .iter()
        .find(|(_, country)| country.tag == tag)
        .map(|(id, _)| id.get())
        .unwrap()
}

#[tokio::test]
async fn lan_host_and_guest_keep_initial_directional_opinions() {
    let scenario = load_embedded_scenario().unwrap();
    let graph = scenario.province_graph().clone();
    let castile = CountryId::new(country(&scenario, "cas"));
    let portugal = CountryId::new(country(&scenario, "por"));
    let scotland = country(&scenario, "sco");
    let scenario = scenario
        .with_opinions([
            (castile, portugal, Opinion::new(37).unwrap()),
            (portugal, castile, Opinion::new(-12).unwrap()),
        ])
        .unwrap();
    let host = LanHost::start(
        HostConfig {
            room: "Opinion snapshot".into(),
            host_country_id: portugal.get(),
            guest_country_id: scotland,
            ai_preset: AiPreset::Balanced,
            grace_minutes: 0,
            port: 0,
        },
        scenario,
    )
    .await
    .unwrap();
    let guest = LanClient::join(host.address(), "Opinion guest".into(), graph)
        .await
        .unwrap();
    let host_view = host.view();
    let guest_view = guest.subscribe();
    let guest_view = guest_view.borrow();
    for game in [&*host_view.game, &*guest_view.game] {
        assert_eq!(
            game.opinion(castile, portugal),
            Some(Opinion::new(37).unwrap())
        );
        assert_eq!(
            game.opinion(portugal, castile),
            Some(Opinion::new(-12).unwrap())
        );
    }
}

#[tokio::test]
#[ignore = "10,000-tick TCP AI soak; run for release acceptance or network/AI changes"]
async fn lan_guest_matches_host_after_ten_thousand_ticks_with_ai() {
    let scenario = load_embedded_scenario().unwrap();
    let graph = scenario.province_graph().clone();
    let host_country_id = country(&scenario, "por");
    let guest_country_id = country(&scenario, "sco");
    let host = LanHost::start(
        HostConfig {
            room: "Replay room".into(),
            host_country_id,
            guest_country_id,
            ai_preset: AiPreset::Aggressive,
            grace_minutes: 0,
            port: 0,
        },
        scenario,
    )
    .await
    .unwrap();
    let guest = LanClient::join(host.address(), "Replay guest".into(), graph)
        .await
        .unwrap();
    let mut received = guest.subscribe();
    assert_eq!(received.borrow().guest_tag, "sco");
    assert!(received.borrow().game.time().is_paused());

    let speed = Command::Time(TimeCommand::SetSpeed(Speed::Three));
    assert!(guest.command(speed.clone()).await.is_err());
    host.command(speed).unwrap();
    guest
        .command(Command::Time(TimeCommand::SetPaused(false)))
        .await
        .unwrap();
    for batch in 0..625 {
        for _ in 0..16 {
            host.tick().unwrap();
        }
        let tick = (batch + 1) * 16;
        timeout(
            WAIT,
            received.wait_for(|view| view.game.time().tick().get() == tick),
        )
        .await
        .unwrap_or_else(|_| panic!("guest did not receive tick {tick}"))
        .unwrap();
    }

    let result = timeout(
        WAIT,
        received.wait_for(|view| view.verified_checksums >= 333),
    )
    .await
    .unwrap()
    .unwrap();
    let authoritative = host.view();
    assert_eq!(result.checksum_mismatches, 0);
    assert_eq!(result.phase, ClientPhase::Connected);
    assert_eq!(result.sequence, authoritative.sequence);
    assert_eq!(result.game.time().speed(), Speed::Three);
    let host_bytes = save_game(&authoritative.game).unwrap();
    let guest_bytes = save_game(&result.game).unwrap();
    assert_eq!(
        canonical_checksum(&guest_bytes),
        canonical_checksum(&host_bytes)
    );
    assert_eq!(guest_bytes, host_bytes);
    assert!(
        result
            .game
            .war_state()
            .unwrap()
            .treaties
            .values()
            .any(|treaty| {
                let humans = [
                    CountryId::new(host_country_id),
                    CountryId::new(guest_country_id),
                ];
                !humans.contains(&treaty.attacker) && !humans.contains(&treaty.defender)
            }),
        "an AI-negotiated treaty should be visible in the replica"
    );
}

#[tokio::test]
async fn lan_metrics_capture_checksums_and_ping_on_both_peers() {
    let scenario = load_embedded_scenario().unwrap();
    let graph = scenario.province_graph().clone();
    let host = LanHost::start(
        HostConfig {
            room: "Metrics test".into(),
            host_country_id: country(&scenario, "por"),
            guest_country_id: country(&scenario, "sco"),
            ai_preset: AiPreset::Balanced,
            grace_minutes: 0,
            port: 0,
        },
        scenario,
    )
    .await
    .unwrap();
    let guest = LanClient::join(host.address(), "Metrics guest".into(), graph)
        .await
        .unwrap();
    guest
        .command(Command::Time(TimeCommand::SetPaused(false)))
        .await
        .unwrap();
    for _ in 0..30 {
        host.tick().unwrap();
    }

    let (host_metrics, guest_metrics) = timeout(WAIT, async {
        loop {
            let host_metrics = host.metrics();
            let guest_metrics = guest.metrics();
            if host_metrics.ping_samples > 0
                && guest_metrics.ping_samples > 0
                && guest_metrics.checksums > 0
            {
                break (host_metrics, guest_metrics);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("both peers should report RTT and the guest should verify a checksum");
    let checksum = canonical_checksum(&save_game(&host.view().game).unwrap());
    for metrics in [host_metrics, guest_metrics] {
        assert_eq!(metrics.checksums, 1);
        assert_eq!(metrics.last_checksum, Some((30, checksum)));
        assert_eq!(metrics.mismatches, 0);
        assert_eq!(metrics.resyncs, 0);
        let mean = metrics.ping_mean_ms.unwrap();
        assert!(metrics.ping_min_ms.unwrap() <= mean);
        assert!(mean <= metrics.ping_max_ms.unwrap());
    }
}

#[tokio::test]
#[ignore = "accelerated 350-year TCP soak, run explicitly"]
async fn lan_guest_matches_host_from_1440_through_1790() {
    const TICKS: u64 = 127_836;
    const CHECKSUM_INTERVAL: u64 = 30;
    const CHECKPOINT_BATCHES: u64 = 100;
    let started = Instant::now();
    let start = Date::new(1440, 1, 1).unwrap();
    let end = Date::new(1790, 1, 1).unwrap();
    assert_eq!(start.checked_add_days(TICKS), Some(end));

    let scenario = load_embedded_scenario().unwrap();
    let graph = scenario.province_graph().clone();
    let host_country_id = country(&scenario, "por");
    let guest_country_id = country(&scenario, "sco");
    let host = LanHost::start(
        HostConfig {
            room: "350-year soak".into(),
            host_country_id,
            guest_country_id,
            ai_preset: AiPreset::Aggressive,
            grace_minutes: 0,
            port: 0,
        },
        scenario,
    )
    .await
    .unwrap();
    let guest = LanClient::join(host.address(), "Soak guest".into(), graph.clone())
        .await
        .unwrap();
    let mut received = guest.subscribe();
    guest
        .command(Command::Time(TimeCommand::SetPaused(false)))
        .await
        .unwrap();

    // Drive an independent copy to surface typed failures and caught AI panics.
    let mut oracle = (*host.view().game).clone();
    let mut decisions = AiDecisionLog::default();
    assert_eq!(oracle.time().date(), start);
    assert!(!oracle.time().is_paused());
    for (country, player) in [(host_country_id, 1), (guest_country_id, 2)] {
        assert_eq!(
            oracle
                .country(CountryId::new(country))
                .unwrap()
                .controller(),
            &Controller::Human(PlayerId::new(player)),
        );
    }
    assert!(oracle
        .countries()
        .any(|(_, country)| matches!(country.controller(), Controller::Ai(_))));

    for first in (0..TICKS).step_by(CHECKSUM_INTERVAL as usize) {
        let target = (first + CHECKSUM_INTERVAL).min(TICKS);
        for tick in first + 1..=target {
            host.tick()
                .unwrap_or_else(|error| panic!("host tick {tick}: {error}"));
            let report = tick_host_with_log(&mut oracle, &mut decisions)
                .unwrap_or_else(|error| panic!("oracle tick {tick}: {error:?}"));
            assert!(
                report.ai_rejections.is_empty()
                    && report.scheduled_failures.is_empty()
                    && report.polity_failures.is_empty()
                    && report.military_failures.is_empty()
                    && decisions.failures() == 0,
                "simulation failure at tick {tick}: {report:?}",
            );
        }

        // Leave the broadcast queue room for the checksum and let the
        // real TCP guest finish this batch before publishing another.
        let current = timeout(
            WAIT,
            received.wait_for(|view| view.game.time().tick().get() == target),
        )
        .await
        .unwrap_or_else(|_| panic!("guest did not receive tick {target}"))
        .unwrap();
        assert_eq!(current.phase, ClientPhase::Connected);
        assert_eq!(current.checksum_mismatches, 0, "tick {target}");

        if target % (CHECKSUM_INTERVAL * CHECKPOINT_BATCHES) == 0 || target == TICKS {
            let authoritative = host.view();
            assert_eq!(current.sequence, authoritative.sequence, "tick {target}");
            let host_bytes = save_game(&authoritative.game).unwrap();
            assert_eq!(
                save_game(&current.game).unwrap(),
                host_bytes,
                "tick {target}"
            );
            assert_eq!(save_game(&oracle).unwrap(), host_bytes, "tick {target}");
            let reloaded = load_game(&host_bytes, graph.clone()).unwrap();
            assert_eq!(save_game(&reloaded).unwrap(), host_bytes, "tick {target}");
        }
    }

    let result = received.borrow();
    let expected_checksums = TICKS / CHECKSUM_INTERVAL;
    assert_eq!(result.game.time().date(), end);
    assert!(
        result.verified_checksums > 0
            && result.verified_checksums + result.revision >= expected_checksums,
        "only {} of {expected_checksums} checksums verified, with {} replacement snapshots",
        result.verified_checksums,
        result.revision,
    );
    assert!(!result.game.war_state().unwrap().treaties.is_empty());
    eprintln!(
        "Accelerated LAN soak: {TICKS} ticks, {} verified checksums, {} mismatches, {} replacement snapshots, {:.2?} elapsed",
        result.verified_checksums,
        result.checksum_mismatches,
        result.revision,
        started.elapsed(),
    );
}

#[tokio::test]
async fn client_requests_snapshot_on_mismatch_and_reconnects_with_original_token() {
    let mut scenario = load_embedded_scenario().unwrap();
    let graph = scenario.province_graph().clone();
    let state = save_game(&scenario).unwrap();
    scenario
        .apply(
            CommandActor::Host(PlayerId::new(1)),
            &Command::Time(TimeCommand::SetSpeed(Speed::Five)),
        )
        .unwrap();
    let corrected = save_game(&scenario).unwrap();
    assert_ne!(state, corrected);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let token = [31; 16];
    let (disconnect, wait_disconnect) = oneshot::channel();
    let (finish, wait_finish) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        assert!(matches!(
            next_request(&mut socket).await,
            ClientMessage::Hello { resume: None, .. }
        ));
        handshake(&mut socket, token, &state, 0).await;
        acknowledge_pause(&mut socket, 1).await;
        write_server(
            &mut socket,
            &ServerMessage::Checksum {
                sequence: 0,
                tick: 0,
                hash: canonical_checksum(&corrected),
            },
        )
        .await
        .unwrap();
        assert_eq!(next_request(&mut socket).await, ClientMessage::Resync);
        write_server(&mut socket, &snapshot(&corrected, 0))
            .await
            .unwrap();
        write_server(
            &mut socket,
            &ServerMessage::Checksum {
                sequence: 0,
                tick: 0,
                hash: canonical_checksum(&corrected),
            },
        )
        .await
        .unwrap();
        wait_disconnect.await.unwrap();
        drop(socket);

        let (mut socket, _) = listener.accept().await.unwrap();
        assert!(
            matches!(next_request(&mut socket).await, ClientMessage::Hello { resume: Some(resume), .. } if resume == token)
        );
        handshake(&mut socket, token, &corrected, 2).await;
        acknowledge_pause(&mut socket, 2).await;
        wait_finish.await.unwrap();
    });

    let client = LanClient::join(address, "Guest".into(), graph)
        .await
        .unwrap();
    let mut view = client.subscribe();
    client
        .command(Command::Time(TimeCommand::SetPaused(true)))
        .await
        .unwrap();
    {
        let result = timeout(WAIT, view.wait_for(|view| view.verified_checksums == 1))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.sequence, 0);
        assert_eq!(result.revision, 1);
        assert_eq!(result.game.time().speed(), Speed::Five);
    }
    let metrics = client.metrics();
    assert_eq!(metrics.mismatches, 1);
    assert_eq!(metrics.resyncs, 1);
    disconnect.send(()).unwrap();
    {
        let result = timeout(
            WAIT,
            view.wait_for(|view| view.sequence == 2 && view.phase == ClientPhase::Connected),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.verified_checksums, 1);
        assert_eq!(result.checksum_mismatches, 1);
        assert_eq!(result.revision, 2);
        assert_eq!(result.game.time().speed(), Speed::Five);
    }
    client
        .command(Command::Time(TimeCommand::SetPaused(true)))
        .await
        .unwrap();
    finish.send(()).unwrap();
    timeout(WAIT, server).await.unwrap().unwrap();
}

#[tokio::test]
async fn host_holds_guest_seat_and_restores_human_control_after_ai_takeover() {
    let scenario = load_embedded_scenario().unwrap();
    let graph = scenario.province_graph().clone();
    let map = scenario.map_identity();
    let guest_country_id = country(&scenario, "sco");
    let host_country_id = country(&scenario, "por");
    let host = LanHost::start(
        HostConfig {
            room: "Reconnect test".into(),
            host_country_id,
            guest_country_id,
            ai_preset: AiPreset::Balanced,
            grace_minutes: 0,
            port: 0,
        },
        scenario,
    )
    .await
    .unwrap();
    let hello = |resume| ClientMessage::Hello {
        version: PROTOCOL_VERSION,
        name: "Guest".into(),
        map_version: map.version.get(),
        map_hash: map.sha256,
        resume,
    };
    let mut peer = TcpStream::connect(host.address()).await.unwrap();
    write_client(&mut peer, &hello(None)).await.unwrap();
    let ServerMessage::Welcome {
        resume,
        player_id: 2,
    } = receive(&mut peer).await
    else {
        panic!("expected welcome");
    };
    assert!(matches!(
        receive(&mut peer).await,
        ServerMessage::Lobby { .. }
    ));
    assert!(matches!(
        receive(&mut peer).await,
        ServerMessage::Snapshot { .. }
    ));
    write_client(&mut peer, &ClientMessage::Confirm { resume })
        .await
        .unwrap();
    let before = save_game(&host.view().game).unwrap();
    write_client(
        &mut peer,
        &ClientMessage::Command {
            seq: 1,
            command: PlayerCommand::SetController {
                country: host_country_id,
                controller: LobbyController::Human { player_id: 2 },
            },
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        receive(&mut peer).await,
        ServerMessage::CommandRejected { seq: 1, .. }
    ));
    assert_eq!(save_game(&host.view().game).unwrap(), before);
    drop(peer);
    let mut status = host.subscribe();
    timeout(
        WAIT,
        status.wait_for(|view| !view.guest_connected && view.can_takeover),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        host.view()
            .game
            .country(CountryId::new(guest_country_id))
            .unwrap()
            .controller(),
        &Controller::Human(PlayerId::new(2))
    );
    for token in [None, Some([0; 16])] {
        let mut impostor = TcpStream::connect(host.address()).await.unwrap();
        write_client(&mut impostor, &hello(token)).await.unwrap();
        assert!(matches!(
            receive(&mut impostor).await,
            ServerMessage::Rejected { .. }
        ));
    }
    host.take_over().unwrap();
    assert!(host.view().temporary_ai);
    assert!(matches!(
        host.view()
            .game
            .country(CountryId::new(guest_country_id))
            .unwrap()
            .controller(),
        Controller::Ai(_)
    ));

    let mut peer = TcpStream::connect(host.address()).await.unwrap();
    write_client(&mut peer, &hello(Some(resume))).await.unwrap();
    assert!(
        matches!(receive(&mut peer).await, ServerMessage::Welcome { resume: accepted, .. } if accepted == resume)
    );
    assert!(matches!(
        receive(&mut peer).await,
        ServerMessage::Lobby { .. }
    ));
    let ServerMessage::Snapshot { state, .. } = receive(&mut peer).await else {
        panic!("expected reconnect snapshot");
    };
    let restored = load_game(&state, graph).unwrap();
    write_client(&mut peer, &ClientMessage::Confirm { resume })
        .await
        .unwrap();
    assert_eq!(
        restored
            .country(CountryId::new(guest_country_id))
            .unwrap()
            .controller(),
        &Controller::Human(PlayerId::new(2))
    );
    assert!(host.view().guest_connected);
    assert!(!host.view().temporary_ai);
}

#[tokio::test]
async fn abandoning_handshake_does_not_reserve_the_guest_seat() {
    let scenario = load_embedded_scenario().unwrap();
    let graph = scenario.province_graph().clone();
    let identity = scenario.map_identity();
    let host = LanHost::start(
        HostConfig {
            room: "Unconfirmed guest".into(),
            host_country_id: country(&scenario, "por"),
            guest_country_id: country(&scenario, "sco"),
            ai_preset: AiPreset::Balanced,
            grace_minutes: 0,
            port: 0,
        },
        scenario,
    )
    .await
    .unwrap();
    let mut abandoned = TcpStream::connect(host.address()).await.unwrap();
    write_client(
        &mut abandoned,
        &ClientMessage::Hello {
            version: PROTOCOL_VERSION,
            name: "Abandoned".into(),
            map_version: identity.version.get(),
            map_hash: identity.sha256,
            resume: None,
        },
    )
    .await
    .unwrap();
    let ServerMessage::Welcome { resume, .. } = receive(&mut abandoned).await else {
        panic!("expected unconfirmed welcome");
    };
    drop(abandoned);
    let mut status = host.subscribe();
    timeout(WAIT, status.wait_for(|view| !view.guest_connected))
        .await
        .unwrap()
        .unwrap();
    let mut retry = TcpStream::connect(host.address()).await.unwrap();
    write_client(
        &mut retry,
        &ClientMessage::Hello {
            version: PROTOCOL_VERSION,
            name: "Retry".into(),
            map_version: identity.version.get(),
            map_hash: identity.sha256,
            resume: Some(resume),
        },
    )
    .await
    .unwrap();
    assert!(
        matches!(receive(&mut retry).await, ServerMessage::Welcome {resume: accepted, ..} if accepted == resume)
    );
    drop(retry);
    timeout(WAIT, status.wait_for(|view| !view.guest_connected))
        .await
        .unwrap()
        .unwrap();
    let guest = LanClient::join(host.address(), "Actual guest".into(), graph)
        .await
        .unwrap();
    assert_eq!(guest.subscribe().borrow().phase, ClientPhase::Connected);
}

async fn receive(socket: &mut TcpStream) -> ServerMessage {
    timeout(WAIT, read_server(socket))
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}

async fn next_request(socket: &mut TcpStream) -> ClientMessage {
    loop {
        let message = read_client(socket).await.unwrap().unwrap();
        match message {
            ClientMessage::Ping { nonce } => write_server(socket, &ServerMessage::Pong { nonce })
                .await
                .unwrap(),
            message => return message,
        }
    }
}

async fn handshake(socket: &mut TcpStream, resume: [u8; 16], state: &[u8], sequence: u64) {
    for message in [
        ServerMessage::Welcome {
            player_id: 2,
            resume,
        },
        ServerMessage::Lobby {
            room: "Fault test".into(),
            countries: Vec::new(),
            guest_tag: "sco".into(),
        },
        snapshot(state, sequence),
    ] {
        write_server(socket, &message).await.unwrap();
    }
    assert_eq!(
        next_request(socket).await,
        ClientMessage::Confirm { resume }
    );
}

fn snapshot(state: &[u8], sequence: u64) -> ServerMessage {
    ServerMessage::Snapshot {
        sequence,
        tick: 0,
        state: state.to_vec(),
    }
}

async fn acknowledge_pause(socket: &mut TcpStream, expected: u64) {
    let request = next_request(socket).await;
    assert_eq!(
        request,
        ClientMessage::PauseRequest {
            seq: expected,
            paused: true
        }
    );
    write_server(socket, &ServerMessage::CommandAccepted { seq: expected })
        .await
        .unwrap();
}
