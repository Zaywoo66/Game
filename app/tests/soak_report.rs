use strategy_app::{run_ai_soak, SOAK_TSV_HEADER};

#[test]
fn yearly_soak_report_is_reproducible_and_machine_readable() {
    let first = run_ai_soak(1, &[1440]).unwrap();
    let second = run_ai_soak(1, &[1440]).unwrap();
    assert_eq!(first.len(), 1);
    let (a, b) = (&first[0], &second[0]);
    assert_eq!((a.seed, a.year, a.ticks), (1440, 1441, 366));
    assert_eq!(a.state_hash, b.state_hash);
    assert_eq!(a.country_count, b.country_count);
    assert_eq!(a.largest_land_provinces, b.largest_land_provinces);
    assert_eq!(a.largest_share_basis_points(), b.largest_share_basis_points());
    assert!(a.largest_share_basis_points() <= 10_000);
    assert!(a.median_tick_ns <= a.p95_tick_ns);
    assert!(a.p95_tick_ns <= a.peak_tick_ns);
    assert_eq!(SOAK_TSV_HEADER.split('\t').count(), 11);
    let row = a.tsv();
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 11);
    assert_eq!(fields[0], "1440");
    assert_eq!(fields[1], "1441");
    assert_eq!(fields[10].len(), 64);
    assert!(fields[10].bytes().all(|byte| byte.is_ascii_hexdigit()));
}

#[test]
fn invalid_soak_arguments_fail_before_launch() {
    assert!(run_ai_soak(0, &[1]).is_err());
    assert!(run_ai_soak(1, &[]).is_err());
}
