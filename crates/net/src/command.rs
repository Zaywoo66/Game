use std::fmt;

use serde::{Deserialize, Serialize};
use strategy_sim::{
    AiProfileKey, ArmyId, CharacterId, Command, Controller, CountryId, FiscalPolicy, Fixed,
    FleetId, NewMonarch, PlayerId, ProvinceId, Sex, Speed, TimeCommand, WarId,
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
    IncreaseStability {
        country: u32,
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
    MoveFleet {
        country: u32,
        fleet: u32,
        destination: u32,
    },
    BuildShip {
        country: u32,
        port: u32,
        sea: u32,
    },
    EmbarkArmy {
        country: u32,
        army: u32,
        fleet: u32,
    },
    DisembarkArmy {
        country: u32,
        fleet: u32,
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
    DeclareIndependence {
        country: u32,
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
    CreateWarFaction {
        country: u32,
        war: u32,
    },
    JoinWarFaction {
        country: u32,
        war: u32,
        leader: u32,
    },
    RenameWarFaction {
        country: u32,
        war: u32,
        name: String,
        dynasty: String,
    },
    AcceptCallToArms {
        country: u32,
        war: u32,
        from: u32,
    },
    RejectCallToArms {
        country: u32,
        war: u32,
        from: u32,
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
    ChooseEvent {
        country: u32,
        key: String,
        option: u32,
    },
    UnlockTechnology {
        country: u32,
        key: String,
    },
    UnlockIdea {
        country: u32,
        key: String,
    },
    UnlockReform {
        country: u32,
        key: String,
    },
    ChangeGovernment {
        country: u32,
        target: String,
        #[serde(default)]
        monarch_name: Option<String>,
        #[serde(default)]
        dynasty: Option<String>,
        #[serde(default)]
        monarch_sex: Option<String>,
    },
    OfferRoyalMarriage {
        country: u32,
        character: u32,
        toward: u32,
        partner: u32,
    },
    AcceptRoyalMarriage {
        country: u32,
        from: u32,
        character: u32,
        partner: u32,
    },
    RejectRoyalMarriage {
        country: u32,
        from: u32,
        character: u32,
        partner: u32,
    },
    Guarantee {
        country: u32,
        protected: u32,
    },
    RevokeGuarantee {
        country: u32,
        protected: u32,
    },
    OfferVassalage {
        country: u32,
        toward: u32,
    },
    AcceptVassalage {
        country: u32,
        from: u32,
    },
    RejectVassalage {
        country: u32,
        from: u32,
    },
    ReleaseVassal {
        country: u32,
        subject: u32,
    },
    OfferTradeRoute {
        country: u32,
        toward: u32,
    },
    AcceptTradeRoute {
        country: u32,
        from: u32,
    },
    RejectTradeRoute {
        country: u32,
        from: u32,
    },
    CloseTradeRoute {
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
            Command::IncreaseStability { country } => Self::IncreaseStability {
                country: country.get(),
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
            Command::MoveFleet {
                country,
                fleet,
                destination,
            } => Self::MoveFleet {
                country: country.get(),
                fleet: fleet.get(),
                destination: destination.get(),
            },
            Command::BuildShip { country, port, sea } => Self::BuildShip {
                country: country.get(),
                port: port.get(),
                sea: sea.get(),
            },
            Command::EmbarkArmy {
                country,
                army,
                fleet,
            } => Self::EmbarkArmy {
                country: country.get(),
                army: army.get(),
                fleet: fleet.get(),
            },
            Command::DisembarkArmy {
                country,
                fleet,
                destination,
            } => Self::DisembarkArmy {
                country: country.get(),
                fleet: fleet.get(),
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
            Command::DeclareIndependence { country } => Self::DeclareIndependence {
                country: country.get(),
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
            Command::CreateWarFaction { country, war } => Self::CreateWarFaction {
                country: country.get(),
                war: war.get(),
            },
            Command::JoinWarFaction {
                country,
                war,
                leader,
            } => Self::JoinWarFaction {
                country: country.get(),
                war: war.get(),
                leader: leader.get(),
            },
            Command::RenameWarFaction {
                country,
                war,
                name,
                dynasty,
            } => Self::RenameWarFaction {
                country: country.get(),
                war: war.get(),
                name: name.clone(),
                dynasty: dynasty.clone(),
            },
            Command::AcceptCallToArms { country, war, from } => Self::AcceptCallToArms {
                country: country.get(),
                war: war.get(),
                from: from.get(),
            },
            Command::RejectCallToArms { country, war, from } => Self::RejectCallToArms {
                country: country.get(),
                war: war.get(),
                from: from.get(),
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
            Command::ChooseEvent {
                country,
                key,
                option,
            } => Self::ChooseEvent {
                country: country.get(),
                key: key.clone(),
                option: *option,
            },
            Command::UnlockTechnology { country, key } => Self::UnlockTechnology {
                country: country.get(),
                key: key.clone(),
            },
            Command::UnlockIdea { country, key } => Self::UnlockIdea {
                country: country.get(),
                key: key.clone(),
            },
            Command::UnlockReform { country, key } => Self::UnlockReform {
                country: country.get(),
                key: key.clone(),
            },
            Command::ChangeGovernment {
                country,
                target,
                monarch,
            } => Self::ChangeGovernment {
                country: country.get(),
                target: target.clone(),
                monarch_name: monarch.as_ref().map(|value| value.name.clone()),
                dynasty: monarch.as_ref().map(|value| value.dynasty.clone()),
                monarch_sex: monarch.as_ref().map(|value| match value.sex {
                    Sex::Male => "male".into(),
                    Sex::Female => "female".into(),
                    Sex::Unknown => "unknown".into(),
                }),
            },
            Command::OfferRoyalMarriage {
                country,
                character,
                toward,
                partner,
            } => Self::OfferRoyalMarriage {
                country: country.get(),
                character: character.get(),
                toward: toward.get(),
                partner: partner.get(),
            },
            Command::AcceptRoyalMarriage {
                country,
                from,
                character,
                partner,
            } => Self::AcceptRoyalMarriage {
                country: country.get(),
                from: from.get(),
                character: character.get(),
                partner: partner.get(),
            },
            Command::RejectRoyalMarriage {
                country,
                from,
                character,
                partner,
            } => Self::RejectRoyalMarriage {
                country: country.get(),
                from: from.get(),
                character: character.get(),
                partner: partner.get(),
            },
            Command::Guarantee { country, protected } => Self::Guarantee {
                country: country.get(),
                protected: protected.get(),
            },
            Command::RevokeGuarantee { country, protected } => Self::RevokeGuarantee {
                country: country.get(),
                protected: protected.get(),
            },
            Command::OfferVassalage { country, toward } => Self::OfferVassalage {
                country: country.get(),
                toward: toward.get(),
            },
            Command::AcceptVassalage { country, from } => Self::AcceptVassalage {
                country: country.get(),
                from: from.get(),
            },
            Command::RejectVassalage { country, from } => Self::RejectVassalage {
                country: country.get(),
                from: from.get(),
            },
            Command::ReleaseVassal { country, subject } => Self::ReleaseVassal {
                country: country.get(),
                subject: subject.get(),
            },
            Command::OfferTradeRoute { country, toward } => Self::OfferTradeRoute {
                country: country.get(),
                toward: toward.get(),
            },
            Command::AcceptTradeRoute { country, from } => Self::AcceptTradeRoute {
                country: country.get(),
                from: from.get(),
            },
            Command::RejectTradeRoute { country, from } => Self::RejectTradeRoute {
                country: country.get(),
                from: from.get(),
            },
            Command::CloseTradeRoute { country, other } => Self::CloseTradeRoute {
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
            PlayerCommand::IncreaseStability { country } => Self::IncreaseStability {
                country: CountryId::new(country),
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
            PlayerCommand::MoveFleet {
                country,
                fleet,
                destination,
            } => Self::MoveFleet {
                country: CountryId::new(country),
                fleet: FleetId::new(fleet),
                destination: ProvinceId::new(destination),
            },
            PlayerCommand::BuildShip { country, port, sea } => Self::BuildShip {
                country: CountryId::new(country),
                port: ProvinceId::new(port),
                sea: ProvinceId::new(sea),
            },
            PlayerCommand::EmbarkArmy {
                country,
                army,
                fleet,
            } => Self::EmbarkArmy {
                country: CountryId::new(country),
                army: ArmyId::new(army),
                fleet: FleetId::new(fleet),
            },
            PlayerCommand::DisembarkArmy {
                country,
                fleet,
                destination,
            } => Self::DisembarkArmy {
                country: CountryId::new(country),
                fleet: FleetId::new(fleet),
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
            PlayerCommand::DeclareIndependence { country } => Self::DeclareIndependence {
                country: CountryId::new(country),
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
            PlayerCommand::CreateWarFaction { country, war } => Self::CreateWarFaction {
                country: CountryId::new(country),
                war: WarId::new(war),
            },
            PlayerCommand::JoinWarFaction {
                country,
                war,
                leader,
            } => Self::JoinWarFaction {
                country: CountryId::new(country),
                war: WarId::new(war),
                leader: CountryId::new(leader),
            },
            PlayerCommand::RenameWarFaction {
                country,
                war,
                name,
                dynasty,
            } => Self::RenameWarFaction {
                country: CountryId::new(country),
                war: WarId::new(war),
                name,
                dynasty,
            },
            PlayerCommand::AcceptCallToArms { country, war, from } => Self::AcceptCallToArms {
                country: CountryId::new(country),
                war: WarId::new(war),
                from: CountryId::new(from),
            },
            PlayerCommand::RejectCallToArms { country, war, from } => Self::RejectCallToArms {
                country: CountryId::new(country),
                war: WarId::new(war),
                from: CountryId::new(from),
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
            PlayerCommand::ChooseEvent {
                country,
                key,
                option,
            } => Self::ChooseEvent {
                country: CountryId::new(country),
                key,
                option,
            },
            PlayerCommand::UnlockTechnology { country, key } => Self::UnlockTechnology {
                country: CountryId::new(country),
                key,
            },
            PlayerCommand::UnlockIdea { country, key } => Self::UnlockIdea {
                country: CountryId::new(country),
                key,
            },
            PlayerCommand::UnlockReform { country, key } => Self::UnlockReform {
                country: CountryId::new(country),
                key,
            },
            PlayerCommand::ChangeGovernment {
                country,
                target,
                monarch_name,
                dynasty,
                monarch_sex,
            } => Self::ChangeGovernment {
                country: CountryId::new(country),
                target,
                monarch: match (monarch_name, dynasty, monarch_sex) {
                    (Some(name), Some(dynasty), Some(sex)) => Some(NewMonarch {
                        name,
                        dynasty,
                        sex: match sex.as_str() {
                            "male" => Sex::Male,
                            "female" => Sex::Female,
                            _ => Sex::Unknown,
                        },
                    }),
                    _ => None,
                },
            },
            PlayerCommand::OfferRoyalMarriage {
                country,
                character,
                toward,
                partner,
            } => Self::OfferRoyalMarriage {
                country: CountryId::new(country),
                character: CharacterId::new(character),
                toward: CountryId::new(toward),
                partner: CharacterId::new(partner),
            },
            PlayerCommand::AcceptRoyalMarriage {
                country,
                from,
                character,
                partner,
            } => Self::AcceptRoyalMarriage {
                country: CountryId::new(country),
                from: CountryId::new(from),
                character: CharacterId::new(character),
                partner: CharacterId::new(partner),
            },
            PlayerCommand::RejectRoyalMarriage {
                country,
                from,
                character,
                partner,
            } => Self::RejectRoyalMarriage {
                country: CountryId::new(country),
                from: CountryId::new(from),
                character: CharacterId::new(character),
                partner: CharacterId::new(partner),
            },
            PlayerCommand::Guarantee { country, protected } => Self::Guarantee {
                country: CountryId::new(country),
                protected: CountryId::new(protected),
            },
            PlayerCommand::RevokeGuarantee { country, protected } => Self::RevokeGuarantee {
                country: CountryId::new(country),
                protected: CountryId::new(protected),
            },
            PlayerCommand::OfferVassalage { country, toward } => Self::OfferVassalage {
                country: CountryId::new(country),
                toward: CountryId::new(toward),
            },
            PlayerCommand::AcceptVassalage { country, from } => Self::AcceptVassalage {
                country: CountryId::new(country),
                from: CountryId::new(from),
            },
            PlayerCommand::RejectVassalage { country, from } => Self::RejectVassalage {
                country: CountryId::new(country),
                from: CountryId::new(from),
            },
            PlayerCommand::ReleaseVassal { country, subject } => Self::ReleaseVassal {
                country: CountryId::new(country),
                subject: CountryId::new(subject),
            },
            PlayerCommand::OfferTradeRoute { country, toward } => Self::OfferTradeRoute {
                country: CountryId::new(country),
                toward: CountryId::new(toward),
            },
            PlayerCommand::AcceptTradeRoute { country, from } => Self::AcceptTradeRoute {
                country: CountryId::new(country),
                from: CountryId::new(from),
            },
            PlayerCommand::RejectTradeRoute { country, from } => Self::RejectTradeRoute {
                country: CountryId::new(country),
                from: CountryId::new(from),
            },
            PlayerCommand::CloseTradeRoute { country, other } => Self::CloseTradeRoute {
                country: CountryId::new(country),
                other: CountryId::new(other),
            },
        })
    }
}
