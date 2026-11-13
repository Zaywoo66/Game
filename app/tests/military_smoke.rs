use strategy_app::{advance_autoplay, new_autoplay};
use strategy_data::{load_game, save_game};
use strategy_sim::Date;

#[test]
#[ignore = "three-seed ten-year AI soak; run for release acceptance or AI balance changes"]
fn three_seeds_complete_ten_years_and_ai_wars_end_in_treaties() {
    let halfway = Date::new(1445, 1, 1).unwrap();
    let end = Date::new(1450, 1, 1).unwrap();
    for seed in [1440, 1441, 1442] {
        let mut game = new_autoplay(seed).unwrap();
        let initial_country_count = game.polity().unwrap().countries.len();
        let first = advance_autoplay(&mut game, halfway).unwrap();
        let bytes = save_game(&game).unwrap();
        let mut reloaded = load_game(&bytes, game.province_graph().clone()).unwrap();
        let second = advance_autoplay(&mut game, end).unwrap();
        let replay = advance_autoplay(&mut reloaded, end).unwrap();
        assert_eq!(second.final_hash, replay.final_hash, "seed {seed}");
        assert_eq!(game, reloaded);
        assert_eq!(first.ticks + second.ticks, 3653);
        assert!(
            second.treaties > 0 && second.cessions > 0,
            "seed {seed}: {second:?}"
        );
        assert!(second.country_count > 0 && second.country_count <= initial_country_count);
    }
}
