use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU32,
};

use strategy_sim::{
    AiProfileKey, AiThresholds, AiWeights, CharacterId, CharacterState, CharacterStats, Command,
    CommandActor, CommandError, Controller, CountryId, CountryPolity, CountryState, Date,
    EconomyState, FiscalAiProfile, FiscalError, FiscalPolicy, Fixed, GameState, MapIdentity,
    ModKey, Modifier, ModifierKind, ModifierSourceId, ModifierStore, MonthlyBudget, PlayerId,
    PolityError, PolityRules, PolityState, PolityTickFailure, Province, ProvinceGraph, ProvinceId,
    ProvinceKind, SimTime, Speed, StateChange, SuccessionState,
};

const ALPHA: CountryId = CountryId::new(1);
const BETA: CountryId = CountryId::new(2);
const ALPHA_LAND: ProvinceId = ProvinceId::new(1);
const BETA_LAND: ProvinceId = ProvinceId::new(2);
const SEA: ProvinceId = ProvinceId::new(3);
const ALPHA_RULER: CharacterId = CharacterId::new(10);
const BETA_RULER: CharacterId = CharacterId::new(20);
const OWNER: CommandActor = CommandActor::Host(PlayerId::new(7));
const OTHER: CommandActor = CommandActor::Guest(PlayerId::new(8));

#[test]
fn validates_ownership_capital_and_current_ai_profile() {
    assert_eq!(
        setup(date(1440, 1, 30), |polity| {
            polity
                .countries
                .get_mut(&BETA)
                .unwrap()
                .provinces
                .insert(ALPHA_LAND);
        }),
        Err(PolityError::SharedProvince {
            province: ALPHA_LAND,
            first: ALPHA,
            second: BETA
        })
    );
    assert_eq!(
        setup(date(1440, 1, 30), |polity| {
            polity.countries.get_mut(&ALPHA).unwrap().capital = BETA_LAND;
        }),
        Err(PolityError::InvalidCapital {
            country: ALPHA,
            capital: BETA_LAND
        })
    );
    assert_eq!(
        setup(date(1440, 1, 30), |polity| {
            polity
                .countries
                .get_mut(&ALPHA)
                .unwrap()
                .provinces
                .insert(SEA);
        }),
        Err(PolityError::InvalidProvince {
            country: ALPHA,
            province: SEA
        })
    );
    assert_eq!(
        setup(date(1440, 1, 30), |polity| {
            polity.ai_profiles.remove("prudent");
        }),
        Err(PolityError::UnknownProfile {
            country: BETA,
            profile: "prudent".to_owned()
        })
    );
}

#[test]
fn fiscal_and_heir_commands_require_the_country_controller() {
    let heir = CharacterId::new(11);
    let mut state = setup(date(1440, 1, 30), |polity| {
        polity
            .characters
            .insert(heir, character(ALPHA, "other_house", date(1420, 1, 1)));
    })
    .unwrap();
    let policy = FiscalPolicy {
        tax_rate: raw(600),
        monthly_spending: raw(900),
    };
    let command = Command::SetFiscalPolicy {
        country: ALPHA,
        policy,
    };
    let nomination = Command::NominateHeir {
        country: ALPHA,
        heir: Some(heir),
    };
    let before = state.clone();

    for actor in [OTHER, CommandActor::Ai(BETA), CommandActor::Ai(ALPHA)] {
        for command in [&command, &nomination] {
            let error = CommandError::NotCountryController(ALPHA);
            assert_eq!(state.validate_command(actor, command), Err(error));
            assert_eq!(state.apply(actor, command), Err(error));
        }
    }
    let invalid = Command::SetFiscalPolicy {
        country: ALPHA,
        policy: FiscalPolicy {
            tax_rate: raw(1_001),
            monthly_spending: Fixed::ZERO,
        },
    };
    assert_eq!(
        state.apply(OWNER, &invalid),
        Err(CommandError::Fiscal(FiscalError::PolicyOutOfRange))
    );
    assert_eq!(
        state.apply(
            OWNER,
            &Command::SetController {
                country: BETA,
                controller: Controller::Ai(AiProfileKey::new("missing")),
            }
        ),
        Err(CommandError::UnknownAiProfile(BETA))
    );
    assert_eq!(state, before);

    assert_eq!(
        state.apply(OWNER, &command).unwrap().changes(),
        &[StateChange::FiscalPolicyChanged {
            country: ALPHA,
            policy
        }]
    );
    assert_eq!(
        state.apply(OWNER, &nomination).unwrap().changes(),
        &[StateChange::SuccessionChanged {
            country: ALPHA,
            succession: SuccessionState {
                ruler: Some(ALPHA_RULER),
                heir: Some(heir)
            },
            regency: false,
        }]
    );
    let beta_policy = Command::SetFiscalPolicy {
        country: BETA,
        policy,
    };
    assert!(state
        .validate_command(CommandActor::Ai(BETA), &beta_policy)
        .is_ok());
    assert_eq!(
        state.validate_command(OWNER, &beta_policy),
        Err(CommandError::NotCountryController(BETA))
    );
}

#[test]
fn month_boundary_taxes_owned_development_and_applies_country_and_ruler_modifiers() {
    let mut state = setup(date(1440, 1, 30), |_| {}).unwrap();
    assert_eq!(state.owner_of_province(ALPHA_LAND), Some(ALPHA));
    assert_eq!(state.owner_of_province(SEA), None);
    assert_eq!(state.country_polity(ALPHA).unwrap().capital, ALPHA_LAND);
    let previous = state.clone();
    let policy = state.country_polity(ALPHA).unwrap().economy.policy;
    let forecast = state.preview_monthly_economy(ALPHA, policy).unwrap();
    assert_eq!(state, previous);
    // Two map development points plus one scenario point, taxed at half.
    // The country and ruler add 200 and 300 raw units respectively.
    assert_eq!(
        forecast.last_month,
        Some(MonthlyBudget {
            income: raw(2_000),
            expenses: raw(1_000)
        })
    );
    assert_eq!(forecast.treasury, raw(1_000));

    let january = state.tick(|_, _| Vec::new()).unwrap();
    assert!(!january
        .delta
        .changes()
        .iter()
        .any(|change| matches!(change, StateChange::EconomyChanged { .. })));
    let february = state.tick(|_, _| Vec::new()).unwrap();
    assert!(february.polity_failures.is_empty());
    assert_eq!(state.country_polity(ALPHA).unwrap().economy, forecast);
    assert!(february
        .delta
        .changes()
        .contains(&StateChange::EconomyChanged {
            country: ALPHA,
            economy: forecast,
        }));
}

#[test]
fn shortfall_caps_paid_expenses_and_bankruptcy_clears_only_after_recovery() {
    let mut state = setup(date(1440, 1, 31), |polity| {
        let economy = &mut polity.countries.get_mut(&ALPHA).unwrap().economy;
        economy.policy = FiscalPolicy {
            tax_rate: Fixed::ZERO,
            monthly_spending: raw(4_000),
        };
    })
    .unwrap();

    advance_to(&mut state, date(1440, 2, 1));
    let economy = state.country_polity(ALPHA).unwrap().economy;
    assert_eq!(economy.treasury, raw(-2_000));
    assert_eq!(
        economy.last_month,
        Some(MonthlyBudget {
            income: raw(500),
            expenses: raw(2_500)
        })
    );
    assert!(economy.bankrupt);

    state
        .apply(
            OWNER,
            &Command::SetFiscalPolicy {
                country: ALPHA,
                policy: FiscalPolicy::default(),
            },
        )
        .unwrap();
    advance_to(&mut state, date(1440, 3, 1));
    let economy = state.country_polity(ALPHA).unwrap().economy;
    assert_eq!(economy.treasury, raw(-1_500));
    assert!(economy.bankrupt);

    state
        .apply(
            OWNER,
            &Command::SetFiscalPolicy {
                country: ALPHA,
                policy: FiscalPolicy {
                    tax_rate: Fixed::ONE,
                    monthly_spending: Fixed::ZERO,
                },
            },
        )
        .unwrap();
    advance_to(&mut state, date(1440, 4, 1));
    let economy = state.country_polity(ALPHA).unwrap().economy;
    assert_eq!(economy.treasury, raw(2_000));
    assert!(!economy.bankrupt);
}

#[test]
fn taxable_development_uses_wide_intermediates_before_scaling() {
    let state = setup(date(1440, 1, 30), |polity| {
        polity.rules.tax_per_development = raw(4_000_000_000_000_000_000);
    })
    .unwrap();
    let economy = state
        .preview_monthly_economy(
            ALPHA,
            FiscalPolicy {
                tax_rate: raw(100),
                monthly_spending: Fixed::ZERO,
            },
        )
        .unwrap();
    assert_eq!(economy.treasury, raw(1_200_000_000_000_000_500));
}

#[test]
fn fiscal_overflow_reports_the_country_and_preserves_its_budget() {
    let mut state = setup(date(1440, 1, 31), |polity| {
        polity.countries.get_mut(&ALPHA).unwrap().economy.treasury = raw(i64::MAX - 500);
    })
    .unwrap();
    let previous = state.country_polity(ALPHA).unwrap().economy;
    assert_eq!(
        state.preview_monthly_economy(ALPHA, previous.policy),
        Err(CommandError::Fiscal(FiscalError::ArithmeticOverflow))
    );

    let report = state.tick(|_, _| Vec::new()).unwrap();
    assert_eq!(
        report.polity_failures,
        [PolityTickFailure {
            country: ALPHA,
            error: FiscalError::ArithmeticOverflow,
        }]
    );
    assert_eq!(state.country_polity(ALPHA).unwrap().economy, previous);
    assert!(state
        .country_polity(BETA)
        .unwrap()
        .economy
        .last_month
        .is_some());
}

#[test]
fn nominated_minor_succeeds_on_death_and_leaves_regency_on_birthday() {
    let heir = CharacterId::new(11);
    let mut state = setup(date(1440, 1, 30), |polity| {
        polity.characters.get_mut(&ALPHA_RULER).unwrap().death = Some(date(1440, 1, 31));
        polity
            .characters
            .insert(heir, character(ALPHA, "other_house", date(1422, 2, 1)));
    })
    .unwrap();
    assert_eq!(state.eligible_heirs(ALPHA), [heir]);
    state
        .apply(
            OWNER,
            &Command::NominateHeir {
                country: ALPHA,
                heir: Some(heir),
            },
        )
        .unwrap();

    let death = state.tick(|_, _| Vec::new()).unwrap();
    assert!(death.delta.changes().contains(&StateChange::CharacterDied {
        character: ALPHA_RULER,
        country: ALPHA,
    }));
    assert!(death
        .delta
        .changes()
        .contains(&StateChange::SuccessionChanged {
            country: ALPHA,
            succession: SuccessionState {
                ruler: Some(heir),
                heir: None
            },
            regency: true,
        }));
    assert_eq!(state.ruler(ALPHA).unwrap().0, heir);
    assert_eq!(
        state
            .character(heir)
            .unwrap()
            .age_years(state.time().date()),
        Some(17)
    );

    let adulthood = state.tick(|_, _| Vec::new()).unwrap();
    assert!(adulthood
        .delta
        .changes()
        .contains(&StateChange::SuccessionChanged {
            country: ALPHA,
            succession: SuccessionState {
                ruler: Some(heir),
                heir: None
            },
            regency: false,
        }));
    assert_eq!(state.is_in_regency(ALPHA), Some(false));
}

#[test]
fn dead_heir_is_skipped_in_favor_of_dynasty_then_oldest_birth_and_id() {
    let heir = CharacterId::new(11);
    let oldest = CharacterId::new(12);
    let mut state = setup(date(1440, 1, 30), |polity| {
        polity.characters.get_mut(&ALPHA_RULER).unwrap().death = Some(date(1440, 1, 31));
        let mut doomed = character(ALPHA, "first_house", date(1410, 1, 1));
        doomed.death = Some(date(1440, 1, 31));
        polity.characters.insert(heir, doomed);
        for (id, dynasty, birth) in [
            (13, "first_house", date(1415, 1, 1)),
            (12, "first_house", date(1415, 1, 1)),
            (14, "other_house", date(1380, 1, 1)),
        ] {
            polity
                .characters
                .insert(CharacterId::new(id), character(ALPHA, dynasty, birth));
        }
        polity.countries.get_mut(&ALPHA).unwrap().succession.heir = Some(heir);
    })
    .unwrap();
    assert_eq!(
        state.apply(
            OWNER,
            &Command::NominateHeir {
                country: ALPHA,
                heir: Some(BETA_RULER)
            }
        ),
        Err(CommandError::InvalidHeir {
            country: ALPHA,
            heir: BETA_RULER
        })
    );
    state.tick(|_, _| Vec::new()).unwrap();
    assert_eq!(
        state.country_polity(ALPHA).unwrap().succession,
        SuccessionState {
            ruler: Some(oldest),
            heir: None,
        }
    );
}

#[test]
fn vacant_country_can_gain_a_ruler_when_a_local_candidate_is_born() {
    let future = CharacterId::new(11);
    let mut state = setup(date(1440, 1, 30), |polity| {
        polity.characters.get_mut(&ALPHA_RULER).unwrap().death = Some(date(1440, 1, 31));
        polity
            .characters
            .insert(future, character(ALPHA, "other_house", date(1440, 2, 2)));
    })
    .unwrap();
    assert!(state.eligible_heirs(ALPHA).is_empty());
    advance_to(&mut state, date(1440, 2, 1));
    assert!(state.ruler(ALPHA).is_none());
    advance_to(&mut state, date(1440, 2, 2));
    assert_eq!(state.ruler(ALPHA).unwrap().0, future);
    assert_eq!(state.is_in_regency(ALPHA), Some(true));
}

#[test]
fn royal_marriage_offer_acceptance_opinion_and_death() {
    let mut state = setup(date(1440, 1, 1), |polity| {
        let bride = polity.characters.get_mut(&BETA_RULER).unwrap();
        bride.sex = strategy_sim::Sex::Female;
        bride.death = Some(date(1440, 1, 3));
    })
    .unwrap();
    let offer = Command::OfferRoyalMarriage {
        country: ALPHA,
        character: ALPHA_RULER,
        toward: BETA,
        partner: BETA_RULER,
    };
    assert!(state
        .apply(OWNER, &offer)
        .unwrap()
        .changes()
        .iter()
        .any(|change| matches!(change, StateChange::MarriageOffered { .. })));
    assert_eq!(
        state.apply(
            OWNER,
            &Command::AcceptRoyalMarriage {
                country: BETA,
                from: ALPHA,
                character: ALPHA_RULER,
                partner: BETA_RULER
            }
        ),
        Err(CommandError::NotCountryController(BETA))
    );
    let accepted = state
        .apply(
            CommandActor::Ai(BETA),
            &Command::AcceptRoyalMarriage {
                country: BETA,
                from: ALPHA,
                character: ALPHA_RULER,
                partner: BETA_RULER,
            },
        )
        .unwrap();
    assert!(accepted
        .changes()
        .iter()
        .any(|change| matches!(change, StateChange::MarriageChanged { active: true, .. })));
    assert_eq!(state.opinion(ALPHA, BETA).unwrap().points(), 0);
    assert_eq!(state.effective_opinion(ALPHA, BETA).unwrap().points(), 20);
    assert_eq!(state.effective_opinion(BETA, ALPHA).unwrap().points(), 20);
    assert_eq!(
        state.character(ALPHA_RULER).unwrap().spouse,
        Some(BETA_RULER)
    );
    advance_to(&mut state, date(1440, 1, 3));
    assert_eq!(state.character(ALPHA_RULER).unwrap().spouse, None);
    assert_eq!(state.effective_opinion(ALPHA, BETA).unwrap().points(), 0);
}

#[test]
fn royal_marriage_rejects_legacy_unknown_sex_and_expires() {
    let mut legacy = setup(date(1440, 1, 1), |polity| {
        polity.characters.get_mut(&BETA_RULER).unwrap().sex = strategy_sim::Sex::Unknown;
    })
    .unwrap();
    assert_eq!(
        legacy.apply(
            OWNER,
            &Command::OfferRoyalMarriage {
                country: ALPHA,
                character: ALPHA_RULER,
                toward: BETA,
                partner: BETA_RULER
            }
        ),
        Err(CommandError::Marriage(
            strategy_sim::MarriageError::UnknownSex(BETA_RULER)
        ))
    );
    let mut state = setup(date(1440, 1, 1), |polity| {
        polity.characters.get_mut(&BETA_RULER).unwrap().sex = strategy_sim::Sex::Female;
    })
    .unwrap();
    state
        .apply(
            OWNER,
            &Command::OfferRoyalMarriage {
                country: ALPHA,
                character: ALPHA_RULER,
                toward: BETA,
                partner: BETA_RULER,
            },
        )
        .unwrap();
    advance_to(&mut state, date(1440, 1, 31));
    assert!(state.diplomacy().unwrap().marriage_offers.is_empty());
    assert_eq!(
        state.apply(
            CommandActor::Ai(BETA),
            &Command::AcceptRoyalMarriage {
                country: BETA,
                from: ALPHA,
                character: ALPHA_RULER,
                partner: BETA_RULER
            }
        ),
        Err(CommandError::Marriage(strategy_sim::MarriageError::NoOffer))
    );
}

#[test]
fn royal_marriage_rejects_minor_and_same_sex_with_typed_reasons() {
    let mut minor = setup(date(1440, 1, 1), |polity| {
        let bride = polity.characters.get_mut(&BETA_RULER).unwrap();
        bride.sex = strategy_sim::Sex::Female;
        bride.birth = date(1425, 1, 1);
    })
    .unwrap();
    let offer = Command::OfferRoyalMarriage {
        country: ALPHA,
        character: ALPHA_RULER,
        toward: BETA,
        partner: BETA_RULER,
    };
    assert_eq!(
        minor.apply(OWNER, &offer),
        Err(CommandError::Marriage(
            strategy_sim::MarriageError::Underage(BETA_RULER)
        ))
    );
    let mut same_sex = setup(date(1440, 1, 1), |_| {}).unwrap();
    assert_eq!(
        same_sex.apply(OWNER, &offer),
        Err(CommandError::Marriage(strategy_sim::MarriageError::SameSex))
    );
}

fn setup(start: Date, edit: impl FnOnce(&mut PolityState)) -> Result<GameState, PolityError> {
    let base = GameState::new(
        SimTime::new(start, Speed::One, false),
        42,
        graph(),
        BTreeMap::from([
            (
                ALPHA,
                CountryState::new(
                    Controller::Human(PlayerId::new(7)),
                    BTreeSet::new(),
                    false,
                    income_grant(200),
                ),
            ),
            (
                BETA,
                CountryState::new(
                    Controller::Ai(AiProfileKey::new("prudent")),
                    BTreeSet::new(),
                    false,
                    ModifierStore::default(),
                ),
            ),
        ]),
        [],
    )
    .unwrap();
    let mut ruler = character(ALPHA, "first_house", date(1400, 1, 1));
    ruler.traits.insert("test_trait".to_owned());
    ruler.modifiers = income_grant(300);
    let profile = FiscalAiProfile {
        weights: AiWeights::default(),
        thresholds: AiThresholds::default(),
        preferred_tax_rate: raw(500),
        preferred_monthly_spending: raw(1_000),
    };
    let mut polity = PolityState {
        rules: PolityRules {
            adulthood_years: 18,
            base_taxable_development_per_province: 1,
            tax_per_development: Fixed::ONE,
            max_tax_rate: Fixed::ONE,
            max_monthly_spending: raw(10_000),
            debt_limit: raw(2_000),
            income_modifier: ModKey::new("income"),
        },
        countries: BTreeMap::from([
            (ALPHA, country("alpha", ALPHA_LAND, ALPHA_RULER)),
            (BETA, country("beta", BETA_LAND, BETA_RULER)),
        ]),
        characters: BTreeMap::from([
            (ALPHA_RULER, ruler),
            (
                BETA_RULER,
                character(BETA, "second_house", date(1410, 1, 1)),
            ),
        ]),
        ai_profiles: BTreeMap::from([
            ("balanced".to_owned(), profile),
            ("prudent".to_owned(), profile),
        ]),
    };
    edit(&mut polity);
    base.with_polity(polity)
}

fn character(country: CountryId, dynasty: &str, birth: Date) -> CharacterState {
    CharacterState {
        sex: strategy_sim::Sex::Male,
        spouse: None,
        name: "Test Character".to_owned(),
        country,
        dynasty: dynasty.to_owned(),
        birth,
        death: None,
        stats: CharacterStats {
            adm: 2,
            dip: 3,
            mil: 4,
        },
        traits: BTreeSet::new(),
        modifiers: ModifierStore::default(),
    }
}

fn country(tag: &str, capital: ProvinceId, ruler: CharacterId) -> CountryPolity {
    CountryPolity {
        tag: tag.to_owned(),
        name: tag.to_owned(),
        color: [0, 0, 0],
        capital,
        provinces: BTreeSet::from([capital]),
        ai_profile: AiProfileKey::new("balanced"),
        economy: EconomyState {
            policy: FiscalPolicy {
                tax_rate: raw(500),
                monthly_spending: raw(1_000),
            },
            ..EconomyState::default()
        },
        succession: SuccessionState {
            ruler: Some(ruler),
            heir: None,
        },
    }
}

fn graph() -> ProvinceGraph {
    ProvinceGraph::try_new(
        MapIdentity {
            version: NonZeroU32::new(1).unwrap(),
            sha256: [0; 32],
        },
        [
            (ALPHA_LAND, ProvinceKind::Land, 2),
            (BETA_LAND, ProvinceKind::Land, 4),
            (SEA, ProvinceKind::Sea, 0),
        ]
        .into_iter()
        .map(|(id, kind, development)| Province {
            id,
            key: format!("province_{}", id.get()),
            kind,
            terrain: "test_terrain".to_owned(),
            centroid: [0, 0],
            development,
            neighbors: Vec::new(),
            ports: Vec::new(),
        }),
    )
    .unwrap()
}

fn income_grant(value: i64) -> ModifierStore {
    let source = ModifierSourceId::new(1);
    let mut store = ModifierStore::default();
    store
        .insert_source(
            source,
            None,
            vec![Modifier {
                source,
                key: ModKey::new("income"),
                kind: ModifierKind::Additive,
                value: raw(value),
            }],
        )
        .unwrap();
    store
}

fn raw(value: i64) -> Fixed {
    Fixed::from_raw(value)
}

fn date(year: u32, month: u8, day: u8) -> Date {
    Date::new(year, month, day).unwrap()
}

fn advance_to(state: &mut GameState, target: Date) {
    while state.time().date() < target {
        let report = state.tick(|_, _| Vec::new()).unwrap();
        assert!(report.polity_failures.is_empty());
    }
}
