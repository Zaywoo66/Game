use strategy_net::{
    canonical_checksum, make_delta, ByteEdit, Replica, ReplicaError, StateDelta, MAX_STATE_BYTES,
};

#[test]
fn sparse_changes_only_include_changed_bytes() {
    let before = b"abc keep this block def";
    let after = b"aBc keep this block xyf";
    let delta = make_delta(4, before, after).unwrap();
    assert_eq!(
        delta.edits,
        [
            ByteEdit {
                offset: 1,
                remove: 1,
                insert: b"B".to_vec()
            },
            ByteEdit {
                offset: 20,
                remove: 2,
                insert: b"xy".to_vec()
            },
        ]
    );
    let mut replica = Replica::new(4, before.to_vec()).unwrap();
    replica.apply(&delta).unwrap();
    assert_eq!(replica.bytes(), after);
    assert_eq!(replica.sequence(), 5);
}

#[test]
fn insertion_and_deletion_preserve_matching_prefix_and_suffix() {
    let before = b"long prefix and suffix";
    let after = b"long prefix a new value and suffix";
    let insert = make_delta(10, before, after).unwrap();
    assert_eq!(insert.edits.len(), 1);
    assert_eq!(insert.edits[0].remove, 0);
    assert_eq!(insert.edits[0].insert.len(), after.len() - before.len());
    let remove = make_delta(11, after, before).unwrap();
    assert_eq!(remove.edits.len(), 1);
    assert!(remove.edits[0].insert.is_empty());
    let mut replica = Replica::new(10, before.to_vec()).unwrap();
    replica.apply(&insert).unwrap();
    assert_eq!(replica.bytes(), after);
    replica.apply(&remove).unwrap();
    assert_eq!(replica.bytes(), before);
    replica
        .apply(&make_delta(12, before, before).unwrap())
        .unwrap();
    assert_eq!(replica.sequence(), 13);
}

#[test]
fn malformed_edits_fail_atomically() {
    let before = b"abcd";
    let mut replica = Replica::new(3, before.to_vec()).unwrap();
    for (offset, remove, insert) in [
        (0, 1, b"a".as_slice()),
        (1, 1, b"a"),
        (5, 1, b"a"),
        (3, u32::MAX, b"a"),
        (3, 0, b""),
    ] {
        let delta = StateDelta {
            base_sequence: 3,
            sequence: 4,
            edits: vec![
                ByteEdit {
                    offset: 1,
                    remove: 1,
                    insert: b"B".to_vec(),
                },
                ByteEdit {
                    offset,
                    remove,
                    insert: insert.to_vec(),
                },
            ],
        };
        assert_eq!(
            replica.apply(&delta),
            Err(ReplicaError::InvalidEdit { index: 1 })
        );
        assert_eq!(replica.sequence(), 3);
        assert_eq!(replica.bytes(), before);
    }
}

#[test]
fn an_authoritative_snapshot_recovers_after_a_missing_delta() {
    let mut replica = Replica::new(1, b"old".to_vec()).unwrap();
    let late = make_delta(2, b"new", b"last").unwrap();
    assert!(matches!(
        replica.apply(&late),
        Err(ReplicaError::InvalidSequence { .. })
    ));
    assert_eq!(replica.bytes(), b"old");
    let mut replica = Replica::new(2, b"new".to_vec()).unwrap();
    replica.apply(&late).unwrap();
    assert_eq!(replica.bytes(), b"last");
    assert_eq!(replica.sequence(), 3);
    assert_eq!(
        make_delta(u64::MAX, b"", b""),
        Err(ReplicaError::SequenceOverflow)
    );
}

#[test]
fn bounds_oversized_snapshots_and_deltas() {
    let oversized = vec![0; MAX_STATE_BYTES + 1];
    assert!(matches!(
        Replica::new(0, oversized),
        Err(ReplicaError::TooLarge)
    ));
    let too_many_bytes = vec![1; MAX_STATE_BYTES];
    assert_eq!(
        make_delta(0, b"", &too_many_bytes),
        Err(ReplicaError::TooLarge)
    );
    let mut replica = Replica::new(0, vec![]).unwrap();
    let delta = StateDelta {
        base_sequence: 0,
        sequence: 1,
        edits: vec![ByteEdit {
            offset: 0,
            remove: 0,
            insert: too_many_bytes,
        }],
    };
    assert_eq!(replica.apply(&delta), Err(ReplicaError::TooLarge));
    assert_eq!(replica.sequence(), 0);
    assert!(replica.bytes().is_empty());
}

#[test]
fn canonical_checksum_matches_versioned_golden_value() {
    let actual: String = canonical_checksum(b"abc")
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        actual,
        "868814196fde78f9e6f1622522701f706a76abcf3757a780ede5fb40a36eeba5"
    );
}

#[test]
fn every_splice_position_round_trips_small_states() {
    let mut sequence = 0;
    for length in 0..24 {
        let before: Vec<u8> = (0..length).map(|index| b'a' + index as u8).collect();
        for offset in 0..=length {
            for removed in 0..=length - offset {
                for insert in [b"".as_slice(), b"X", b"XYZ"] {
                    let mut after = before.clone();
                    after.splice(offset..offset + removed, insert.iter().copied());
                    let mut replica = Replica::new(sequence, before.clone()).unwrap();
                    replica
                        .apply(&make_delta(sequence, &before, &after).unwrap())
                        .unwrap();
                    assert_eq!(replica.bytes(), after);
                    sequence += 1;
                }
            }
        }
    }
}
