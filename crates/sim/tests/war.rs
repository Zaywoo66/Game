use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU32,
};

use strategy_sim::{
    AiProfileKey, AiThresholds, AiWeights, Army, ArmyId, Command, CommandActor, CommandError,
    Controller, CountryId, CountryPolity, CountryState, Date, DiplomacyError, EconomyState,
    EdgeKind, FiscalAiProfile, Fixed, GameState, MapIdentity, ModKey, ModifierStore, Opinion,
    PlayerId, PolityRules, PolityState, Province, ProvinceEdge, ProvinceGraph, ProvinceId,
    ProvinceKind, SimTime, Speed, StateChange, SuccessionState, TickReport, WarError, WarId,
    WarRules, WarState,
};

#[test]
fn alliance_offer_accept_break_and_calendar_cooldown() {
    let mut state = setup(|_| {});
    let offer = Command::OfferAlliance {
        country: A,
        toward: B,
    };
    assert_eq!(
        state.validate_command(GUEST, &offer),
        Err(CommandError::NotCountryController(A))
    );
    apply(&mut state, HOST, offer.clone());
    assert_eq!(
        state.validate_command(HOST, &offer),
        Err(CommandError::Diplomacy(DiplomacyError::OfferAlreadyPending))
    );
    apply(
        &mut state,
        GUEST,
        Command::AcceptAlliance {
            country: B,
            from: A,
        },
    );
    assert!(state.allied(A, B));
    assert_eq!(
        state.validate_command(HOST, &offer),
        Err(CommandError::Diplomacy(DiplomacyError::AlreadyAllied))
    );
    apply(
        &mut state,
        HOST,
        Command::BreakAlliance {
            country: A,
            other: B,
        },
    );
    assert!(!state.allied(A, B));
    assert_eq!(state.opinion(A, B).unwrap().points(), -25);
    assert_eq!(state.opinion(B, A).unwrap().points(), -25);
    assert_eq!(
        state.validate_command(HOST, &offer),
        Err(CommandError::Diplomacy(DiplomacyError::Cooldown(
            Date::new(1440, 3, 1).unwrap()
        )))
    );
    for _ in 0..60 {
        day(&mut state);
    }
    assert!(state.validate_command(HOST, &offer).is_ok());
}

#[test]
fn alliance_rejection_and_expiry_are_deterministic() {
    let mut state = setup(|_| {})
        .with_opinions([(A, B, Opinion::new(100).unwrap())])
        .unwrap();
    apply(
        &mut state,
        HOST,
        Command::OfferAlliance {
            country: A,
            toward: B,
        },
    );
    apply(
        &mut state,
        GUEST,
        Command::RejectAlliance {
            country: B,
            from: A,
        },
    );
    assert!(state.diplomacy().unwrap().offers.is_empty());
    apply(
        &mut state,
        HOST,
        Command::OfferAlliance {
            country: A,
            toward: B,
        },
    );
    for _ in 0..30 {
        day(&mut state);
    }
    assert!(state.diplomacy().unwrap().offers.is_empty());
    assert_eq!(
        state.validate_command(
            GUEST,
            &Command::AcceptAlliance {
                country: B,
                from: A
            }
        ),
        Err(CommandError::Diplomacy(DiplomacyError::NoOffer))
    );
}

#[test]
fn common_threat_requires_borders_to_both_and_strictly_more_soldiers() {
    let base = setup(|_| {});
    let mut countries: BTreeMap<_, _> = base
        .countries()
        .map(|(id, state)| (id, state.clone()))
        .collect();
    countries.insert(C, base.country(A).unwrap().clone());
    let mut polity = base.polity().unwrap().clone();
    polity
        .countries
        .get_mut(&A)
        .unwrap()
        .provinces
        .remove(&province(3));
    let mut third = polity.countries[&A].clone();
    third.tag = "third".to_owned();
    third.name = "Third".to_owned();
    third.capital = province(3);
    third.provinces = BTreeSet::from([province(3)]);
    polity.countries.insert(C, third);
    let mut military = base.war_state().unwrap().clone();
    military.armies.insert(
        ArmyId::new(1),
        Army {
            country: C,
            province: province(3),
            soldiers: 200,
            morale: Fixed::ONE,
            movement: None,
        },
    );
    military.armies.insert(
        ArmyId::new(2),
        Army {
            country: A,
            province: province(1),
            soldiers: 100,
            morale: Fixed::ONE,
            movement: None,
        },
    );
    military.armies.insert(
        ArmyId::new(3),
        Army {
            country: B,
            province: province(4),
            soldiers: 100,
            morale: Fixed::ONE,
            movement: None,
        },
    );
    military.next_army_id = 4;
    let mut tied_military = military.clone();
    tied_military
        .armies
        .get_mut(&ArmyId::new(2))
        .unwrap()
        .soldiers = 200;
    let tied = GameState::new(
        *base.time(),
        base.seed(),
        base.province_graph().clone(),
        countries.clone(),
        [],
    )
    .unwrap()
    .with_polity(polity.clone())
    .unwrap()
    .with_war_state(tied_military)
    .unwrap();
    assert!(!tied.common_stronger_neighbor(A, B));
    let state = GameState::new(
        *base.time(),
        base.seed(),
        base.province_graph().clone(),
        countries,
        [],
    )
    .unwrap()
    .with_polity(polity)
    .unwrap()
    .with_war_state(military)
    .unwrap();
    assert!(state.common_stronger_neighbor(A, B));
    assert!(!state.common_stronger_neighbor(A, C));
}

#[test]
fn direct_wartime_enemies_cannot_form_an_alliance() {
    let mut state = setup(|_| {});
    apply(
        &mut state,
        HOST,
        Command::DeclareWar {
            country: A,
            defender: B,
            goal: province(5),
        },
    );
    assert_eq!(
        state.validate_command(
            HOST,
            &Command::OfferAlliance {
                country: A,
                toward: B
            }
        ),
        Err(CommandError::Diplomacy(DiplomacyError::DirectWar))
    );
}

#[test]
fn postwar_truce_does_not_block_a_new_alliance() {
    let mut state = setup(|_| {});
    apply(
        &mut state,
        HOST,
        Command::DeclareWar {
            country: A,
            defender: B,
            goal: province(5),
        },
    );
    apply(
        &mut state,
        HOST,
        Command::OfferPeace {
            country: A,
            war: WAR,
            cede_goal: false,
        },
    );
    apply(
        &mut state,
        GUEST,
        Command::AcceptPeace {
            country: B,
            war: WAR,
        },
    );
    assert!(state
        .validate_command(
            HOST,
            &Command::OfferAlliance {
                country: A,
                toward: B
            }
        )
        .is_ok());
}

const A: CountryId = CountryId::new(1);
const B: CountryId = CountryId::new(2);
const C: CountryId = CountryId::new(3);
const HOST: CommandActor = CommandActor::Host(PlayerId::new(1));
const GUEST: CommandActor = CommandActor::Guest(PlayerId::new(2));
const FIRST_ARMY: ArmyId = ArmyId::new(1);
const SECOND_ARMY: ArmyId = ArmyId::new(2);
const WAR: WarId = WarId::new(1);

#[test]
fn weighted_astar_selects_cheapest_typed_legs_and_advances_in_whole_ticks() {
    let mut state = setup(|_| {});
    let path = state.find_army_path(A, province(1), province(3)).unwrap();
    assert_eq!(
        path.iter()
            .map(|leg| (leg.to, leg.kind, leg.cost))
            .collect::<Vec<_>>(),
        [
            (province(2), EdgeKind::Normal, 2),
            (province(3), EdgeKind::River, 3)
        ]
    );
    assert!(state.find_army_path(A, province(1), province(5)).is_none());
    assert_eq!(
        state.find_army_path(A, province(1), province(1)),
        Some(Vec::new())
    );

    raise(&mut state, HOST, A, 1, 100);
    apply(
        &mut state,
        HOST,
        Command::MoveArmy {
            country: A,
            army: FIRST_ARMY,
            destination: province(3),
        },
    );
    day(&mut state);
    let army = &state.war_state().unwrap().armies[&FIRST_ARMY];
    assert_eq!(army.province, province(1));
    assert_eq!(army.movement.as_ref().unwrap().progress(), raw(500));
    day(&mut state);
    assert_eq!(
        state.war_state().unwrap().armies[&FIRST_ARMY].province,
        province(2)
    );
    for _ in 0..3 {
        day(&mut state);
    }
    let army = &state.war_state().unwrap().armies[&FIRST_ARMY];
    assert_eq!(army.province, province(3));
    assert!(army.movement.is_none());
}

#[test]
fn rejects_zero_asymmetric_and_duplicate_typed_costs() {
    let graph = |first_cost, second_cost, duplicate| {
        let mut first = node(1, &[(2, EdgeKind::River, first_cost)]);
        if duplicate {
            first.neighbors.push(ProvinceEdge {
                to: province(2),
                kind: EdgeKind::River,
                cost: first_cost + 1,
            });
        }
        ProvinceGraph::try_new(
            identity(),
            [first, node(2, &[(1, EdgeKind::River, second_cost)])],
        )
    };
    assert!(graph(0, 0, false).is_err());
    assert!(graph(2, 3, false).is_err());
    assert!(graph(2, 2, true).is_err());
    assert!(graph(2, 2, false).is_ok());
}

#[test]
fn raising_splitting_merging_and_rejections_are_atomic() {
    let mut state = setup(|_| {});
    let before = state.clone();
    let forbidden = Command::RaiseArmy {
        country: A,
        province: province(1),
        soldiers: 100,
    };
    assert_eq!(
        state.apply(GUEST, &forbidden),
        Err(CommandError::NotCountryController(A))
    );
    assert_eq!(
        state.apply(
            HOST,
            &Command::RaiseArmy {
                country: A,
                province: province(1),
                soldiers: 0
            }
        ),
        Err(CommandError::War(WarError::InvalidArmySize))
    );
    assert_eq!(state, before);

    apply(&mut state, HOST, forbidden);
    assert_eq!(
        state.country_polity(A).unwrap().economy.treasury,
        raw(99_000)
    );
    apply(
        &mut state,
        HOST,
        Command::SplitArmy {
            country: A,
            army: FIRST_ARMY,
            soldiers: 1,
        },
    );
    assert_eq!(state.war_state().unwrap().armies[&FIRST_ARMY].soldiers, 99);
    assert_eq!(state.war_state().unwrap().armies[&SECOND_ARMY].soldiers, 1);
    apply(
        &mut state,
        HOST,
        Command::MergeArmies {
            country: A,
            into: FIRST_ARMY,
            from: SECOND_ARMY,
        },
    );
    let war = state.war_state().unwrap();
    assert_eq!(war.armies.len(), 1);
    assert_eq!(war.armies[&FIRST_ARMY].soldiers, 100);
    assert_eq!(war.next_army_id, 3);
}

#[test]
fn a_scripted_war_resolves_battle_siege_and_occupation_before_peace_transfers_the_goal() {
    let mut state = setup(|_| {});
    raise(&mut state, HOST, A, 3, 100);
    raise(&mut state, GUEST, B, 4, 50);
    assert_eq!(
        state.validate_command(
            HOST,
            &Command::DeclareWar {
                country: A,
                defender: B,
                goal: province(4)
            }
        ),
        Err(CommandError::War(WarError::InvalidWarGoal(province(4))))
    );
    apply(
        &mut state,
        HOST,
        Command::DeclareWar {
            country: A,
            defender: B,
            goal: province(5),
        },
    );
    assert!(state.country_at_war(A) && state.country_at_war(B));
    assert_eq!(
        state.validate_command(
            HOST,
            &Command::OfferPeace {
                country: A,
                war: WAR,
                cede_goal: true
            }
        ),
        Err(CommandError::War(WarError::InvalidPeaceTerms))
    );
    apply(
        &mut state,
        HOST,
        Command::MoveArmy {
            country: A,
            army: FIRST_ARMY,
            destination: province(5),
        },
    );
    let battle = day(&mut state);
    assert!(battle
        .delta
        .changes()
        .contains(&StateChange::BattleResolved {
            war: WAR,
            province: province(4),
            winner: A,
            loser: B,
            winner_casualties: 5,
            loser_casualties: 10,
        }));
    let armies = &state.war_state().unwrap().armies;
    assert_eq!(armies[&FIRST_ARMY].soldiers, 95);
    assert_eq!(armies[&SECOND_ARMY].soldiers, 40);
    assert_eq!(armies[&SECOND_ARMY].province, province(5));
    assert_eq!(armies[&SECOND_ARMY].morale, raw(600));

    apply(
        &mut state,
        GUEST,
        Command::MoveArmy {
            country: B,
            army: SECOND_ARMY,
            destination: province(6),
        },
    );
    apply(
        &mut state,
        HOST,
        Command::MoveArmy {
            country: A,
            army: FIRST_ARMY,
            destination: province(5),
        },
    );
    day(&mut state);
    assert_eq!(state.war_state().unwrap().siege_days(province(5)), 0);
    for expected in 1..=3 {
        day(&mut state);
        let war = state.war_state().unwrap();
        assert_eq!(war.siege_days(province(5)), expected);
        assert_eq!(war.siege_required(province(5)), 4);
        assert_eq!(war.occupation(province(5)), None);
    }
    day(&mut state);
    assert_eq!(state.war_state().unwrap().occupation(province(5)), Some(A));
    assert_eq!(state.war_state().unwrap().wars[&WAR].score, 40);
    assert_eq!(state.owner_of_province(province(5)), Some(B));

    apply(
        &mut state,
        HOST,
        Command::OfferPeace {
            country: A,
            war: WAR,
            cede_goal: true,
        },
    );
    assert_eq!(
        state.validate_command(
            HOST,
            &Command::AcceptPeace {
                country: A,
                war: WAR
            }
        ),
        Err(CommandError::War(WarError::CannotAcceptOwnOffer))
    );
    let peace = state
        .apply(
            GUEST,
            &Command::AcceptPeace {
                country: B,
                war: WAR,
            },
        )
        .unwrap();
    assert!(peace.changes().contains(&StateChange::PeaceConcluded {
        war: WAR,
        attacker: A,
        defender: B,
        ceded: Some(province(5)),
    }));
    assert!(peace
        .changes()
        .contains(&StateChange::ProvinceOwnershipChanged {
            province: province(5),
            owner: A
        }));
    assert_eq!(state.owner_of_province(province(5)), Some(A));
    assert!(!state.country_at_war(A) && !state.country_at_war(B));
    assert_eq!(
        state.war_state().unwrap().treaties[&WAR].ceded,
        Some(province(5))
    );
    assert_eq!(
        state.validate_command(
            GUEST,
            &Command::DeclareWar {
                country: B,
                defender: A,
                goal: province(5)
            }
        ),
        Err(CommandError::War(WarError::TruceActive))
    );
}

#[test]
fn splitting_does_not_create_more_supply_for_the_same_country_and_province() {
    let mut state = setup(|rules| {
        rules.base_supply = 4;
        rules.attrition_rate = raw(500);
    });
    raise(&mut state, HOST, A, 1, 10);
    apply(
        &mut state,
        HOST,
        Command::SplitArmy {
            country: A,
            army: FIRST_ARMY,
            soldiers: 6,
        },
    );
    day(&mut state);
    let armies = &state.war_state().unwrap().armies;
    assert_eq!(armies[&FIRST_ARMY].soldiers, 4);
    assert_eq!(armies[&SECOND_ARMY].soldiers, 3);
}

fn apply(state: &mut GameState, actor: CommandActor, command: Command) {
    state.apply(actor, &command).unwrap();
}

fn raise(
    state: &mut GameState,
    actor: CommandActor,
    country: CountryId,
    where_id: u32,
    soldiers: u32,
) {
    apply(
        state,
        actor,
        Command::RaiseArmy {
            country,
            province: province(where_id),
            soldiers,
        },
    );
}

fn day(state: &mut GameState) -> TickReport {
    let report = state.tick(|_, _| Vec::new()).unwrap();
    assert!(report.military_failures.is_empty());
    report
}

fn setup(tune: impl FnOnce(&mut WarRules)) -> GameState {
    let graph = ProvinceGraph::try_new(
        identity(),
        [
            node(1, &[(2, EdgeKind::Normal, 2), (3, EdgeKind::Strait, 9)]),
            node(
                2,
                &[
                    (1, EdgeKind::Normal, 2),
                    (3, EdgeKind::River, 3),
                    (6, EdgeKind::Port, 1),
                ],
            ),
            node(
                3,
                &[
                    (1, EdgeKind::Strait, 9),
                    (2, EdgeKind::River, 3),
                    (4, EdgeKind::Normal, 1),
                ],
            ),
            node(
                4,
                &[
                    (3, EdgeKind::Normal, 1),
                    (5, EdgeKind::Normal, 2),
                    (6, EdgeKind::Normal, 1),
                ],
            ),
            node(5, &[(4, EdgeKind::Normal, 2), (6, EdgeKind::Normal, 1)]),
            node(
                6,
                &[
                    (2, EdgeKind::Port, 1),
                    (4, EdgeKind::Normal, 1),
                    (5, EdgeKind::Normal, 1),
                ],
            ),
        ],
    )
    .unwrap();
    let country = |id| {
        CountryState::new(
            Controller::Human(PlayerId::new(id)),
            BTreeSet::new(),
            false,
            ModifierStore::default(),
        )
    };
    let polity_country = |name: &str, capital, provinces: &[u32]| CountryPolity {
        tag: name.to_owned(),
        name: name.to_owned(),
        color: [0, 0, 0],
        capital: province(capital),
        provinces: provinces.iter().copied().map(province).collect(),
        ai_profile: AiProfileKey::new("test_profile"),
        economy: EconomyState {
            treasury: raw(100_000),
            ..EconomyState::default()
        },
        succession: SuccessionState::default(),
    };
    let polity = PolityState {
        rules: PolityRules {
            adulthood_years: 18,
            base_taxable_development_per_province: 1,
            tax_per_development: Fixed::ONE,
            max_tax_rate: Fixed::ONE,
            max_monthly_spending: Fixed::ZERO,
            debt_limit: Fixed::ZERO,
            income_modifier: ModKey::new("income"),
        },
        countries: BTreeMap::from([
            (A, polity_country("first", 1, &[1, 2, 3])),
            (B, polity_country("second", 4, &[4, 5, 6])),
        ]),
        characters: BTreeMap::new(),
        ai_profiles: BTreeMap::from([(
            "test_profile".to_owned(),
            FiscalAiProfile {
                weights: AiWeights::default(),
                thresholds: AiThresholds::default(),
                preferred_tax_rate: Fixed::ZERO,
                preferred_monthly_spending: Fixed::ZERO,
            },
        )]),
    };
    let mut rules = WarRules {
        raise_cost_per_soldier: raw(10),
        max_army_size: 500,
        max_morale: Fixed::ONE,
        morale_recovery_per_tick: raw(100),
        retreat_morale_threshold: raw(200),
        winner_casualty_rate: raw(50),
        loser_casualty_rate: raw(200),
        battle_morale_loss: raw(400),
        base_supply: 500,
        supply_per_development: 0,
        attrition_rate: Fixed::ZERO,
        siege_days_per_level: 2,
        battle_score: 10,
        occupation_score: 30,
        peace_score: 40,
        truce_ticks: 180,
        combat_modifier: ModKey::new("army_combat"),
        morale_modifier: ModKey::new("army_morale"),
    };
    tune(&mut rules);
    GameState::new(
        SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false),
        42,
        graph,
        BTreeMap::from([(A, country(1)), (B, country(2))]),
        [],
    )
    .unwrap()
    .with_polity(polity)
    .unwrap()
    .with_war_state(WarState::new(rules, BTreeMap::from([(province(5), 2)])))
    .unwrap()
}

fn node(id: u32, edges: &[(u32, EdgeKind, u32)]) -> Province {
    Province {
        id: province(id),
        key: format!("province_{id}"),
        kind: ProvinceKind::Land,
        terrain: "test_land".into(),
        centroid: [id as i32, 0],
        development: 0,
        neighbors: edges
            .iter()
            .map(|&(to, kind, cost)| ProvinceEdge {
                to: province(to),
                kind,
                cost,
            })
            .collect(),
        ports: Vec::new(),
    }
}

fn identity() -> MapIdentity {
    MapIdentity {
        version: NonZeroU32::new(2).unwrap(),
        sha256: [0; 32],
    }
}

fn province(id: u32) -> ProvinceId {
    ProvinceId::new(id)
}

fn raw(value: i64) -> Fixed {
    Fixed::from_raw(value)
}
#[test]
fn retreat_is_for_threatened_armies_and_peace_withdraws_remaining_foreign_armies() {
    let mut state = setup(|rules| rules.retreat_morale_threshold = Fixed::ONE);
    raise(&mut state, HOST, A, 3, 100);
    apply(
        &mut state,
        HOST,
        Command::DeclareWar {
            country: A,
            defender: B,
            goal: province(5),
        },
    );
    assert_eq!(
        state.validate_command(
            HOST,
            &Command::RetreatArmy {
                country: A,
                army: FIRST_ARMY,
                destination: province(2),
            }
        ),
        Err(CommandError::War(WarError::InvalidRetreat(province(2))))
    );
    let advance = Command::MoveArmy {
        country: A,
        army: FIRST_ARMY,
        destination: province(4),
    };
    apply(&mut state, HOST, advance.clone());
    let report = day(&mut state);
    assert!(report.delta.changes().iter().any(|change| matches!(
        change, StateChange::ArmyChanged { army: FIRST_ARMY, state: Some(army) }
            if army.province == province(4)
    )));
    assert_eq!(state.war_state().unwrap().siege_days(province(4)), 1);
    let retreat = state
        .apply(
            HOST,
            &Command::RetreatArmy {
                country: A,
                army: FIRST_ARMY,
                destination: province(3),
            },
        )
        .unwrap();
    assert!(retreat.changes().iter().any(|change| matches!(
        change, StateChange::ArmyChanged { army: FIRST_ARMY, state: Some(army) }
            if army.province == province(3) && army.movement.is_none()
    )));
    assert_eq!(state.war_state().unwrap().siege_days(province(4)), 0);

    apply(&mut state, HOST, advance);
    day(&mut state);
    apply(
        &mut state,
        HOST,
        Command::MoveArmy {
            country: A,
            army: FIRST_ARMY,
            destination: province(5),
        },
    );
    apply(
        &mut state,
        GUEST,
        Command::OfferPeace {
            country: B,
            war: WAR,
            cede_goal: false,
        },
    );
    let peace = state
        .apply(
            HOST,
            &Command::AcceptPeace {
                country: A,
                war: WAR,
            },
        )
        .unwrap();
    assert!(peace.changes().contains(&StateChange::PeaceConcluded {
        war: WAR,
        attacker: A,
        defender: B,
        ceded: None,
    }));
    assert!(peace.changes().iter().any(|change| matches!(
        change, StateChange::ArmyChanged { army: FIRST_ARMY, state: Some(army) }
            if army.province == province(1) && army.movement.is_none()
    )));
    assert!(state.war_state().unwrap().sieges.is_empty());
    assert_eq!(state.owner_of_province(province(5)), Some(B));
    day(&mut state);
    let army = &state.war_state().unwrap().armies[&FIRST_ARMY];
    assert_eq!(army.province, province(1));
    assert!(army.movement.is_none());
}
