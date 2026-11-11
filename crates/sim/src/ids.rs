macro_rules! id_type {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(value: u32) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u32 {
                self.0
            }
        }
    };
}

id_type!(ProvinceId, "Stable numeric identifier for a province.");
id_type!(CountryId, "Stable numeric identifier for a country.");
id_type!(CharacterId, "Stable numeric identifier for a character.");
id_type!(ArmyId, "Stable numeric identifier for an army.");
id_type!(PlayerId, "Stable numeric identifier for a human player.");
