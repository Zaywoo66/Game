use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU32,
};

use strategy_sim::{
    AiProfileKey, AiThresholds, AiWeights, Army, ArmyId, Command, CommandActor, CommandError,
    Controller, CountryId, CountryPolity, CountryState, Date, EconomyState, EdgeKind,
    FiscalAiProfile, FiscalPolicy, Fixed, Fleet, FleetId, GameState, MapIdentity, ModKey,
    ModifierStore, NavyError, NavyState, PlayerId, PolityRules, PolityState, Province,
    ProvinceEdge, ProvinceGraph, ProvinceId, ProvinceKind, SimTime, Speed, StateChange,
    SuccessionState, WarRules, WarState,
};

const A: CountryId = CountryId::new(1);
const B: CountryId = CountryId::new(2);
const HOST: CommandActor = CommandActor::Host(PlayerId::new(1));
const ARMY: ArmyId = ArmyId::new(1);
const FLEET: FleetId = FleetId::new(1);

fn province(
    id: u32,
    kind: ProvinceKind,
    edges: &[(u32, EdgeKind, u32)],
    ports: &[u32],
) -> Province {
    Province {
        id: ProvinceId::new(id),
        key: format!("province_{id}"),
        kind,
        terrain: "plains".into(),
        centroid: [id as i32, 0],
        development: 1,
        neighbors: edges
            .iter()
            .map(|&(to, kind, cost)| ProvinceEdge {
                to: ProvinceId::new(to),
                kind,
                cost,
            })
            .collect(),
        ports: ports.iter().copied().map(ProvinceId::new).collect(),
    }
}

fn setup(ships: u32) -> GameState {
    setup_with_enemy(ships, false)
}

fn setup_with_enemy(ships: u32, enemy_at_port: bool) -> GameState {
    let graph = ProvinceGraph::try_new(
        MapIdentity {
            version: NonZeroU32::new(3).unwrap(),
            sha256: [7; 32],
        },
        [
            province(1, ProvinceKind::Land, &[(3, EdgeKind::Port, 1)], &[3]),
            province(
                2,
                ProvinceKind::Land,
                &[(4, EdgeKind::Port, 1), (5, EdgeKind::Normal, 1)],
                &[4],
            ),
            province(
                3,
                ProvinceKind::Sea,
                &[(1, EdgeKind::Port, 1), (4, EdgeKind::Normal, 2)],
                &[],
            ),
            province(
                4,
                ProvinceKind::Sea,
                &[(3, EdgeKind::Normal, 2), (2, EdgeKind::Port, 1)],
                &[],
            ),
            province(5, ProvinceKind::Land, &[(2, EdgeKind::Normal, 1)], &[]),
        ],
    )
    .unwrap();
    let base = |player| {
        CountryState::new(
            Controller::Human(PlayerId::new(player)),
            BTreeSet::new(),
            false,
            ModifierStore::default(),
        )
    };
    let nation = |tag: &str, capital: u32, provinces: &[u32]| CountryPolity {
        tag: tag.into(),
        government: "monarchy".into(),
        name: tag.into(),
        color: [0, 0, 0],
        capital: ProvinceId::new(capital),
        provinces: provinces.iter().copied().map(ProvinceId::new).collect(),
        ai_profile: AiProfileKey::new("test_profile"),
        economy: EconomyState {
            treasury: Fixed::from_raw(100_000),
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
            (A, nation("first", 1, &[1])),
            (B, nation("second", 5, &[2, 5])),
        ]),
        characters: BTreeMap::new(),
        ai_profiles: BTreeMap::from([(
            "test_profile".into(),
            FiscalAiProfile {
                weights: AiWeights::default(),
                thresholds: AiThresholds::default(),
                preferred_tax_rate: Fixed::ZERO,
                preferred_monthly_spending: Fixed::ZERO,
                tech_branch_weights: BTreeMap::new(),
                idea_priority: 0,
                reform_priority: 0,
            },
        )]),
    };
    let rules = WarRules {
        raise_cost_per_soldier: Fixed::ZERO,
        max_army_size: 5000,
        max_morale: Fixed::ONE,
        morale_recovery_per_tick: Fixed::ZERO,
        retreat_morale_threshold: Fixed::from_raw(100),
        winner_casualty_rate: Fixed::ZERO,
        loser_casualty_rate: Fixed::from_raw(100),
        battle_morale_loss: Fixed::from_raw(100),
        base_supply: 5000,
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
    let mut war = WarState::new(rules, BTreeMap::new());
    war.armies.insert(
        ARMY,
        Army {
            country: A,
            province: ProvinceId::new(1),
            soldiers: 1000,
            morale: Fixed::ONE,
            movement: None,
        },
    );
    war.armies.insert(
        ArmyId::new(2),
        Army {
            country: B,
            province: ProvinceId::new(2),
            soldiers: 500,
            morale: Fixed::ONE,
            movement: None,
        },
    );
    war.next_army_id = 3;
    let mut navy = NavyState::new();
    navy.fleets.insert(
        FLEET,
        Fleet {
            country: A,
            sea: ProvinceId::new(3),
            ships,
            movement: None,
            cargo: None,
        },
    );
    if enemy_at_port {
        navy.fleets.insert(
            FleetId::new(2),
            Fleet {
                country: B,
                sea: ProvinceId::new(3),
                ships: 1,
                movement: None,
                cargo: None,
            },
        );
    }
    navy.next_fleet_id = if enemy_at_port { 3 } else { 2 };
    GameState::new(
        SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false),
        42,
        graph,
        BTreeMap::from([(A, base(1)), (B, base(2))]),
        [],
    )
    .unwrap()
    .with_polity(polity)
    .unwrap()
    .with_war_state(war)
    .unwrap()
    .with_navy(navy)
    .unwrap()
}

#[test]
fn an_unopposed_fleet_closes_port_tax_until_it_sails_away() {
    let mut state = setup(1);
    let policy = FiscalPolicy {
        tax_rate: Fixed::ONE,
        monthly_spending: Fixed::ZERO,
    };
    state
        .apply(
            CommandActor::Host(PlayerId::new(2)),
            &Command::SetFiscalPolicy { country: B, policy },
        )
        .unwrap();
    let income = |state: &GameState| {
        state
            .preview_monthly_economy(B, policy)
            .unwrap()
            .last_month
            .unwrap()
            .income
    };
    assert_eq!(income(&state), Fixed::from_raw(4_000));
    state
        .apply(
            HOST,
            &Command::DeclareWar {
                country: A,
                defender: B,
                goal: ProvinceId::new(2),
            },
        )
        .unwrap();
    state
        .apply(
            HOST,
            &Command::MoveFleet {
                country: A,
                fleet: FLEET,
                destination: ProvinceId::new(4),
            },
        )
        .unwrap();
    for _ in 0..2 {
        state.tick(|_, _| vec![]).unwrap();
    }
    assert!(state.is_port_blockaded(ProvinceId::new(2)));
    assert!(!state.is_port_blockaded(ProvinceId::new(5)));
    assert_eq!(income(&state), Fixed::from_raw(2_000));
    while state.time().date() != Date::new(1440, 2, 1).unwrap() {
        state.tick(|_, _| vec![]).unwrap();
    }
    assert_eq!(
        state
            .country_polity(B)
            .unwrap()
            .economy
            .last_month
            .unwrap()
            .income,
        Fixed::from_raw(2_000)
    );
    state
        .apply(
            HOST,
            &Command::MoveFleet {
                country: A,
                fleet: FLEET,
                destination: ProvinceId::new(3),
            },
        )
        .unwrap();
    for _ in 0..2 {
        state.tick(|_, _| vec![]).unwrap();
    }
    assert!(!state.is_port_blockaded(ProvinceId::new(2)));
    assert_eq!(income(&state), Fixed::from_raw(4_000));

    state
        .apply(
            HOST,
            &Command::MoveFleet {
                country: A,
                fleet: FLEET,
                destination: ProvinceId::new(4),
            },
        )
        .unwrap();
    for _ in 0..2 {
        state.tick(|_, _| vec![]).unwrap();
    }
    assert!(state.is_port_blockaded(ProvinceId::new(2)));
    let war = *state.war_state().unwrap().wars.keys().next().unwrap();
    state
        .apply(
            HOST,
            &Command::OfferPeace {
                country: A,
                war,
                cede_goal: false,
            },
        )
        .unwrap();
    state
        .apply(
            CommandActor::Host(PlayerId::new(2)),
            &Command::AcceptPeace { country: B, war },
        )
        .unwrap();
    assert!(!state.is_port_blockaded(ProvinceId::new(2)));
    assert_eq!(income(&state), Fixed::from_raw(4_000));
}

#[test]
fn same_side_fleet_in_the_zone_prevents_a_blockade() {
    let mut state = setup_with_enemy(1, true);
    state
        .apply(
            HOST,
            &Command::DeclareWar {
                country: A,
                defender: B,
                goal: ProvinceId::new(2),
            },
        )
        .unwrap();
    // B's fleet sits beside A's port, but A's own fleet defends that sea zone.
    assert!(!state.is_port_blockaded(ProvinceId::new(1)));
}

#[test]
fn hostile_fleet_interrupts_landing_until_naval_battle_clears_the_port() {
    let mut state = setup_with_enemy(2, true);
    state
        .apply(
            HOST,
            &Command::DeclareWar {
                country: A,
                defender: B,
                goal: ProvinceId::new(2),
            },
        )
        .unwrap();
    state
        .apply(
            HOST,
            &Command::EmbarkArmy {
                country: A,
                army: ARMY,
                fleet: FLEET,
            },
        )
        .unwrap();
    let landing = Command::DisembarkArmy {
        country: A,
        fleet: FLEET,
        destination: ProvinceId::new(1),
    };
    assert_eq!(
        state.validate_command(HOST, &landing),
        Err(CommandError::Navy(NavyError::HostileFleet(FleetId::new(2))))
    );
    let report = state.tick(|_, _| vec![]).unwrap();
    assert!(report
        .delta
        .changes()
        .iter()
        .any(|change| matches!(change, StateChange::NavalBattleResolved(_))));
    assert!(!state.navy().unwrap().fleets.contains_key(&FleetId::new(2)));
    assert!(state.validate_command(HOST, &landing).is_ok());
}

#[test]
fn naval_invasion_moves_cargo_across_disconnected_land_and_fights_after_landing() {
    let mut state = setup(1);
    state
        .apply(
            HOST,
            &Command::DeclareWar {
                country: A,
                defender: B,
                goal: ProvinceId::new(2),
            },
        )
        .unwrap();
    state
        .apply(
            HOST,
            &Command::EmbarkArmy {
                country: A,
                army: ARMY,
                fleet: FLEET,
            },
        )
        .unwrap();
    assert_eq!(state.navy().unwrap().fleets[&FLEET].cargo, Some(ARMY));
    assert_eq!(
        state.validate_command(
            HOST,
            &Command::MoveArmy {
                country: A,
                army: ARMY,
                destination: ProvinceId::new(1)
            }
        ),
        Err(CommandError::Navy(NavyError::CargoAlreadyAboard(ARMY)))
    );
    state
        .apply(
            HOST,
            &Command::MoveFleet {
                country: A,
                fleet: FLEET,
                destination: ProvinceId::new(4),
            },
        )
        .unwrap();
    state.tick(|_, _| vec![]).unwrap();
    assert_eq!(state.navy().unwrap().fleets[&FLEET].sea, ProvinceId::new(3));
    state.tick(|_, _| vec![]).unwrap();
    assert_eq!(state.navy().unwrap().fleets[&FLEET].sea, ProvinceId::new(4));
    assert_eq!(state.war_state().unwrap().armies[&ARMY].soldiers, 1000);
    state
        .apply(
            HOST,
            &Command::DisembarkArmy {
                country: A,
                fleet: FLEET,
                destination: ProvinceId::new(2),
            },
        )
        .unwrap();
    assert_eq!(
        state.war_state().unwrap().armies[&ARMY].province,
        ProvinceId::new(2)
    );
    let report = state.tick(|_, _| vec![]).unwrap();
    assert!(report.delta.changes().iter().any(|change| matches!(change,
        StateChange::BattleResolved { province, .. } if *province == ProvinceId::new(2))));
}

#[test]
fn fleet_capacity_and_port_requirements_reject_bad_commands_atomically() {
    let state = setup(1);
    let before = state.clone();
    assert_eq!(
        state.validate_command(
            HOST,
            &Command::DisembarkArmy {
                country: A,
                fleet: FLEET,
                destination: ProvinceId::new(2)
            }
        ),
        Err(CommandError::Navy(NavyError::NoCargo(FLEET)))
    );
    assert_eq!(
        state.validate_command(
            HOST,
            &Command::MoveFleet {
                country: A,
                fleet: FLEET,
                destination: ProvinceId::new(5)
            }
        ),
        Err(CommandError::Navy(NavyError::InvalidSea(ProvinceId::new(
            5
        ))))
    );
    assert_eq!(state, before);
}
