use strategy_ai::{decide, AiContext};
use strategy_app::load_embedded_modern_scenario;
use strategy_data::{load_game, save_game};
use strategy_sim::{Command, CommandActor, Controller, Date, PlayerId, ProvinceKind, TimeCommand};

#[test]
fn modern_2022_scenario_uses_sovereign_owners_for_all_land() {
    let game = load_embedded_modern_scenario().expect("modern scenario should load");

    assert_eq!(game.time().start_date(), Date::new(2022, 1, 1).unwrap());
    assert_eq!(game.countries().count(), 90);

    let (uk, _) = game
        .countries()
        .find(|(country, _)| game.country_polity(*country).unwrap().tag == "modern_gb1")
        .expect("United Kingdom should be present");
    let uk_polity = game.country_polity(uk).unwrap();
    assert_eq!(
        uk_polity.capital,
        game.province_graph().by_key("london").unwrap().id
    );

    for (country, _) in game.countries() {
        let capital = game.country_polity(country).unwrap().capital;
        assert_eq!(game.owner_of_province(capital), Some(country));
    }

    let land_count = game
        .province_graph()
        .iter()
        .filter(|province| province.kind == ProvinceKind::Land)
        .count();
    let owned_land_count: usize = game
        .countries()
        .map(|(country, _)| {
            game.country_polity(country)
                .expect("every country has a polity")
                .provinces
                .len()
        })
        .sum();
    assert_eq!(owned_land_count, land_count);
    assert_eq!(land_count, 3468);

    let (saudi_arabia, _) = game
        .countries()
        .find(|(country, _)| game.country_polity(*country).unwrap().tag == "modern_sau")
        .expect("Saudi Arabia should be present");
    assert_eq!(
        game.ruler(saudi_arabia).unwrap().1.name,
        "Salman bin Abdulaziz Al Saud"
    );
    let saudi_ideas = game
        .ideas()
        .expect("modern scenario should load national ideas");
    assert!(saudi_ideas
        .definitions
        .values()
        .any(|idea| { idea.country == "modern_sau" && idea.key == "arab_caliphate_restoration" }));

    let human_countries: Vec<_> = game
        .countries()
        .filter(|(_, state)| matches!(state.controller(), Controller::Human(_)))
        .map(|(id, _)| game.country_polity(id).unwrap().tag.as_str())
        .collect();
    assert_eq!(human_countries, ["modern_gb1"]);

    let owner_tag = |key: &str| {
        let province = game
            .province_graph()
            .by_key(key)
            .expect("province key should exist");
        let owner = game
            .owner_of_province(province.id)
            .expect("land province should be owned");
        game.country_polity(owner).unwrap().tag.clone()
    };
    for city in ["geneva", "basel", "lugano", "schaffhausen", "bellinzona"] {
        assert_eq!(
            owner_tag(city),
            "modern_che",
            "{city} should belong to Switzerland"
        );
    }
    for city in ["prague", "brno", "ostrava"] {
        assert_eq!(
            owner_tag(city),
            "modern_cze",
            "{city} should belong to Czechia"
        );
    }
    for city in ["lviv", "kyiv", "sevastopol", "kerch"] {
        assert_eq!(
            owner_tag(city),
            "modern_ukr",
            "{city} should belong to Ukraine"
        );
    }
    assert_eq!(owner_tag("bregenz"), "modern_aut");
    assert_eq!(owner_tag("beirut"), "modern_lbn");
    let (lebanon, _) = game
        .countries()
        .find(|(country, _)| game.country_polity(*country).unwrap().tag == "modern_lbn")
        .expect("Lebanon should be present");
    assert_eq!(game.ruler(lebanon).unwrap().1.name, "Michel Aoun");
}

#[test]
fn modern_ai_does_not_start_wars_while_relations_are_neutral() {
    let game = load_embedded_modern_scenario().expect("modern scenario should load");
    for (country, state) in game.countries() {
        if !matches!(state.controller(), Controller::Ai(_)) {
            continue;
        }
        let mut context = AiContext::new(&game, country);
        let commands = decide(&game, country, &mut context);
        assert!(!commands
            .iter()
            .any(|(command, _)| matches!(command, Command::DeclareWar { .. })));
    }
}

#[test]
fn modern_army_recruitment_takes_thirty_days_and_blocks_orders_until_ready() {
    let mut game = load_embedded_modern_scenario().expect("modern scenario should load");
    let (country, _) = game
        .countries()
        .find(|(_, state)| state.controller() == &Controller::Human(PlayerId::new(1)))
        .expect("host should control one campaign country");
    let capital = game.country_polity(country).unwrap().capital;
    game.apply(
        CommandActor::Host(PlayerId::new(1)),
        &Command::RaiseArmy {
            country,
            province: capital,
            soldiers: 100,
        },
    )
    .expect("the human country should be able to recruit");
    let army_id = *game
        .war_state()
        .unwrap()
        .armies
        .iter()
        .find(|(_, army)| army.country == country && army.recruitment_days_remaining == 30)
        .map(|(id, _)| id)
        .expect("new army should be marked as recruiting");
    assert_eq!(
        game.war_state().unwrap().armies[&army_id].recruitment_days_remaining,
        30
    );
    assert!(game
        .apply(
            CommandActor::Host(PlayerId::new(1)),
            &Command::MoveArmy {
                country,
                army: army_id,
                destination: capital
            },
        )
        .is_err());

    let bytes = save_game(&game).expect("recruitment state should be saved");
    let restored = load_game(&bytes, game.province_graph().clone())
        .expect("recruitment state should be restored");
    assert_eq!(
        restored.war_state().unwrap().armies[&army_id].recruitment_days_remaining,
        30
    );

    game.apply(
        CommandActor::Host(PlayerId::new(1)),
        &Command::Time(TimeCommand::SetPaused(false)),
    )
    .expect("host can unpause");
    for _ in 0..29 {
        game.tick(|_, _| Vec::new()).expect("daily tick");
    }
    assert_eq!(
        game.war_state().unwrap().armies[&army_id].recruitment_days_remaining,
        1
    );
    game.tick(|_, _| Vec::new()).expect("final training day");
    assert_eq!(
        game.war_state().unwrap().armies[&army_id].recruitment_days_remaining,
        0
    );
}
