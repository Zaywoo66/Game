use strategy_sim::{Trigger, TriggerView};

struct CountryView {
    technologies: Vec<&'static str>,
    at_war: bool,
}

impl TriggerView for CountryView {
    fn has_technology(&self, key: &str) -> bool {
        self.technologies.contains(&key)
    }

    fn is_at_war(&self) -> bool {
        self.at_war
    }
}

#[test]
fn nested_conditions_use_only_the_subject_country() {
    let trigger = Trigger::All(vec![
        Trigger::Any(vec![
            Trigger::CountryHasTech("mil_2".into()),
            Trigger::CountryHasTech("mil_3".into()),
        ]),
        Trigger::Not(Box::new(Trigger::AtWar(true))),
    ]);
    let mut view = CountryView {
        technologies: vec!["mil_3"],
        at_war: false,
    };

    assert!(trigger.evaluate(&view));
    view.at_war = true;
    assert!(!trigger.evaluate(&view));
    view.at_war = false;
    view.technologies.clear();
    assert!(!trigger.evaluate(&view));
}

#[test]
fn empty_combinations_and_war_operands_have_boolean_semantics() {
    let view = CountryView {
        technologies: Vec::new(),
        at_war: false,
    };

    assert!(Trigger::All(Vec::new()).evaluate(&view));
    assert!(!Trigger::Any(Vec::new()).evaluate(&view));
    assert!(Trigger::AtWar(false).evaluate(&view));
    assert!(!Trigger::AtWar(true).evaluate(&view));
}

#[test]
fn composition_short_circuits_in_declaration_order() {
    struct WarOnly;

    impl TriggerView for WarOnly {
        fn has_technology(&self, _: &str) -> bool {
            panic!("the first operand should decide the result");
        }

        fn is_at_war(&self) -> bool {
            true
        }
    }

    let technology = Trigger::CountryHasTech("mil_3".into());
    assert!(!Trigger::All(vec![Trigger::AtWar(false), technology.clone()]).evaluate(&WarOnly));
    assert!(Trigger::Any(vec![Trigger::AtWar(true), technology]).evaluate(&WarOnly));
}
