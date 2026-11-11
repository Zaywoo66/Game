/// The simulation facts a trigger can read for its subject country.
pub trait TriggerView {
    fn has_technology(&self, key: &str) -> bool;
    fn is_at_war(&self) -> bool;
}

/// A condition evaluated against a read-only view of one country.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trigger {
    All(Vec<Self>),
    Any(Vec<Self>),
    Not(Box<Self>),
    CountryHasTech(String),
    AtWar(bool),
}

impl Trigger {
    /// Evaluates in declaration order and stops as soon as the result is known.
    pub fn evaluate(&self, view: &(impl TriggerView + ?Sized)) -> bool {
        match self {
            Self::All(conditions) => conditions.iter().all(|condition| condition.evaluate(view)),
            Self::Any(conditions) => conditions.iter().any(|condition| condition.evaluate(view)),
            Self::Not(condition) => !condition.evaluate(view),
            Self::CountryHasTech(key) => view.has_technology(key),
            Self::AtWar(expected) => view.is_at_war() == *expected,
        }
    }
}
