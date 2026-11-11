use std::time::Instant;

use strategy_app::{new_autoplay, tick_host_with_log, AiDecisionLog};
use strategy_data::save_game;
use strategy_net::{canonical_checksum, make_delta, ServerMessage, MAX_SERVER_FRAME_BYTES};

fn wire_size(message: &ServerMessage) -> usize {
    postcard::to_allocvec(message).unwrap().len() + 4
}

/// Manual 365-day wire and tick sample for the current embedded map.
#[test]
#[ignore = "run when recording the M5b.3 density measurements"]
fn one_year_of_authoritative_updates_on_the_dense_map() {
    let mut game = new_autoplay(1440).unwrap();
    let mut decisions = AiDecisionLog::default();
    let mut previous = save_game(&game).unwrap();
    let snapshot_bytes = wire_size(&ServerMessage::Snapshot {
        sequence: 0,
        tick: 0,
        state: previous.clone(),
    });
    assert!(snapshot_bytes < MAX_SERVER_FRAME_BYTES);

    let mut daily_bytes = 0_usize;
    let mut checksum_bytes = 0_usize;
    let mut fallback_snapshots = 0_usize;
    let mut tick_ns = Vec::with_capacity(365);
    for sequence in 1..=365_u64 {
        let began = Instant::now();
        let report = tick_host_with_log(&mut game, &mut decisions).unwrap();
        assert!(report.ai_rejections.is_empty());
        assert!(report.military_failures.is_empty());
        tick_ns.push(began.elapsed().as_nanos());

        let current = save_game(&game).unwrap();
        let message = match make_delta(sequence - 1, &previous, &current) {
            Ok(changes) => ServerMessage::Delta {
                tick: sequence,
                changes,
            },
            Err(_) => {
                fallback_snapshots += 1;
                ServerMessage::Snapshot {
                    sequence,
                    tick: sequence,
                    state: current.clone(),
                }
            }
        };
        daily_bytes += wire_size(&message);
        if sequence % 30 == 0 {
            checksum_bytes += wire_size(&ServerMessage::Checksum {
                sequence,
                tick: sequence,
                hash: canonical_checksum(&current),
            });
        }
        previous = current;
    }
    tick_ns.sort_unstable();
    println!(
        "M5b.3 map={} provinces; full_snapshot_wire_bytes={snapshot_bytes}; one_year_daily_update_wire_bytes={daily_bytes}; one_year_checksum_wire_bytes={checksum_bytes}; one_year_total_wire_bytes={}; fallback_snapshots={fallback_snapshots}; median_host_tick_ns={}",
        game.province_graph().iter().len(), daily_bytes + checksum_bytes, tick_ns[tick_ns.len() / 2],
    );
    assert_eq!(game.time().tick().get(), 365);
}
