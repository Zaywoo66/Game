use std::fmt;

use serde::{Deserialize, Serialize};
use strategy_sim::{
    AiProfileKey, ArmyId, CharacterId, Command, Controller, CountryId, FiscalPolicy, Fixed,
    PlayerId, ProvinceId, Speed, TimeCommand, WarId,
};

use crate::LobbyController;

/// Wire command representation. Roles and player identity are assigned by the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerCommand {
    SetPaused {
        paused: bool,
    },
    SetSpeed {
        speed: u8,
    },
    SetController {
        country: u32,
        controller: LobbyController,
    },
    SetFiscalPolicy {
        country: u32,
        tax_rate: i64,
        monthly_spending: i64,
    },
    NominateHeir {
        country: u32,
        heir: Option<u32>,
    },
    RaiseArmy {
        country: u32,
        province: u32,
        soldiers: u32,
    },
    MergeArmies {
        country: u32,
        into: u32,
        from: u32,
    },
    SplitArmy {
        country: u32,
        army: u32,
        soldiers: u32,
    },
    MoveArmy {
        country: u32,
        army: u32,
        destination: u32,
    },
    RetreatArmy {
        country: u32,
        army: u32,
        destination: u32,
    },
    DeclareWar {
        country: u32,
        defender: u32,
        goal: u32,
    },
    OfferPeace {
        country: u32,
        war: u32,
        cede_goal: bool,
    },
    AcceptPeace {
        country: u32,
        war: u32,
    },
    OfferAlliance {
        country: u32,
        toward: u32,
    },
    AcceptAlliance {
        country: u32,
        from: u32,
    },
    RejectAlliance {
        country: u32,
        from: u32,
    },
    BreakAlliance {
        country: u32,
        other: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandDecodeError {
    InvalidSpeed(u8),
}

impl fmt::Display for CommandDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSpeed(speed) => write!(f, "speed {speed} is outside the range 1–5"),
        }
    }
}

impl std::error::Error for CommandDecodeError {}

impl From<&Controller> for LobbyController {
    fn from(controller: &Controller) -> Self {
        match controller {
            Controller::Human(player) => Self::Human {
                player_id: player.get(),
            },
            Controller::Ai(profile) => Self::Ai {
                profile: profile.as_str().to_owned(),
            },
        }
    }
}

impl From<LobbyController> for Controller {
    fn from(controller: LobbyController) -> Self {
        match controller {
            LobbyController::Human { player_id } => Self::Human(PlayerId::new(player_id)),
            LobbyController::Ai { profile } => Self::Ai(AiProfileKey::new(profile)),
        }
    }
}

impl From<&Command> for PlayerCommand {
    fn from(command: &Command) -> Self {
        match command {
            Command::Time(TimeCommand::SetPaused(paused)) => Self::SetPaused { paused: *paused },
            Command::Time(TimeCommand::SetSpeed(speed)) => Self::SetSpeed { speed: speed.get() },
            Command::SetController {
                country,
                controller,
            } => Self::SetController {
                country: country.get(),
                controller: controller.into(),
            },
            Command::SetFiscalPolicy { country, policy } => Self::SetFiscalPolicy {
                country: country.get(),
                tax_rate: policy.tax_rate.raw(),
                monthly_spending: policy.monthly_spending.raw(),
            },
            Command::NominateHeir { country, heir } => Self::NominateHeir {
                country: country.get(),
                heir: heir.map(CharacterId::get),
            },
            Command::RaiseArmy {
                country,
                province,
                soldiers,
            } => Self::RaiseArmy {
                country: country.get(),
                province: province.get(),
                soldiers: *soldiers,
            },
            Command::MergeArmies {
                country,
                into,
                from,
            } => Self::MergeArmies {
                country: country.get(),
                into: into.get(),
                from: from.get(),
            },
            Command::SplitArmy {
                country,
                army,
                soldiers,
            } => Self::SplitArmy {
                country: country.get(),
                army: army.get(),
                soldiers: *soldiers,
            },
            Command::MoveArmy {
                country,
                army,
                destination,
            } => Self::MoveArmy {
                country: country.get(),
                army: army.get(),
                destination: destination.get(),
            },
            Command::RetreatArmy {
                country,
                army,
                destination,
            } => Self::RetreatArmy {
                country: country.get(),
                army: army.get(),
                destination: destination.get(),
            },
            Command::DeclareWar {
                country,
                defender,
                goal,
            } => Self::DeclareWar {
                country: country.get(),
                defender: defender.get(),
                goal: goal.get(),
            },
            Command::OfferPeace {
                country,
                war,
                cede_goal,
            } => Self::OfferPeace {
                country: country.get(),
                war: war.get(),
                cede_goal: *cede_goal,
            },
            Command::AcceptPeace { country, war } => Self::AcceptPeace {
                country: country.get(),
                war: war.get(),
            },
            Command::OfferAlliance { country, toward } => Self::OfferAlliance {
                country: country.get(),
                toward: toward.get(),
            },
            Command::AcceptAlliance { country, from } => Self::AcceptAlliance {
                country: country.get(),
                from: from.get(),
            },
            Command::RejectAlliance { country, from } => Self::RejectAlliance {
                country: country.get(),
                from: from.get(),
            },
            Command::BreakAlliance { country, other } => Self::BreakAlliance {
                country: country.get(),
                other: other.get(),
            },
        }
    }
}

impl From<Command> for PlayerCommand {
    fn from(command: Command) -> Self {
        Self::from(&command)
    }
}

impl TryFrom<PlayerCommand> for Command {
    type Error = CommandDecodeError;

    fn try_from(command: PlayerCommand) -> Result<Self, Self::Error> {
        Ok(match command {
            PlayerCommand::SetPaused { paused } => Self::Time(TimeCommand::SetPaused(paused)),
            PlayerCommand::SetSpeed { speed } => Self::Time(TimeCommand::SetSpeed(
                Speed::try_from(speed).map_err(|_| CommandDecodeError::InvalidSpeed(speed))?,
            )),
            PlayerCommand::SetController {
                country,
                controller,
            } => Self::SetController {
                country: CountryId::new(country),
                controller: controller.into(),
            },
            PlayerCommand::SetFiscalPolicy {
                country,
                tax_rate,
                monthly_spending,
            } => Self::SetFiscalPolicy {
                country: CountryId::new(country),
                policy: FiscalPolicy {
                    tax_rate: Fixed::from_raw(tax_rate),
                    monthly_spending: Fixed::from_raw(monthly_spending),
                },
            },
            PlayerCommand::NominateHeir { country, heir } => Self::NominateHeir {
                country: CountryId::new(country),
                heir: heir.map(CharacterId::new),
            },
            PlayerCommand::RaiseArmy {
                country,
                province,
                soldiers,
            } => Self::RaiseArmy {
                country: CountryId::new(country),
                province: ProvinceId::new(province),
                soldiers,
            },
            PlayerCommand::MergeArmies {
                country,
                into,
                from,
            } => Self::MergeArmies {
                country: CountryId::new(country),
                into: ArmyId::new(into),
                from: ArmyId::new(from),
            },
            PlayerCommand::SplitArmy {
                country,
                army,
                soldiers,
            } => Self::SplitArmy {
                country: CountryId::new(country),
                army: ArmyId::new(army),
                soldiers,
            },
            PlayerCommand::MoveArmy {
                country,
                army,
                destination,
            } => Self::MoveArmy {
                country: CountryId::new(country),
                army: ArmyId::new(army),
                destination: ProvinceId::new(destination),
            },
            PlayerCommand::RetreatArmy {
                country,
                army,
                destination,
            } => Self::RetreatArmy {
                country: CountryId::new(country),
                army: ArmyId::new(army),
                destination: ProvinceId::new(destination),
            },
            PlayerCommand::DeclareWar {
                country,
                defender,
                goal,
            } => Self::DeclareWar {
                country: CountryId::new(country),
                defender: CountryId::new(defender),
                goal: ProvinceId::new(goal),
            },
            PlayerCommand::OfferPeace {
                country,
                war,
                cede_goal,
            } => Self::OfferPeace {
                country: CountryId::new(country),
                war: WarId::new(war),
                cede_goal,
            },
            PlayerCommand::AcceptPeace { country, war } => Self::AcceptPeace {
                country: CountryId::new(country),
                war: WarId::new(war),
            },
            PlayerCommand::OfferAlliance { country, toward } => Self::OfferAlliance {
                country: CountryId::new(country),
                toward: CountryId::new(toward),
            },
            PlayerCommand::AcceptAlliance { country, from } => Self::AcceptAlliance {
                country: CountryId::new(country),
                from: CountryId::new(from),
            },
            PlayerCommand::RejectAlliance { country, from } => Self::RejectAlliance {
                country: CountryId::new(country),
                from: CountryId::new(from),
            },
            PlayerCommand::BreakAlliance { country, other } => Self::BreakAlliance {
                country: CountryId::new(country),
                other: CountryId::new(other),
            },
        })
    }
}
