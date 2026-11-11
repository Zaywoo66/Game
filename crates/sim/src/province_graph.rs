use std::{collections::BTreeMap, fmt, num::NonZeroU32};

use crate::ProvinceId;

/// Identity recorded in saves to detect changes to the loaded map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapIdentity {
    pub version: NonZeroU32,
    pub sha256: [u8; 32],
}

impl MapIdentity {
    /// Call on the loaded map before accepting state from a save.
    pub fn verify_save(self, saved: Self) -> Result<(), MapIdentityMismatch> {
        if self == saved {
            Ok(())
        } else {
            Err(MapIdentityMismatch {
                loaded: self,
                saved,
            })
        }
    }
}

impl fmt::Display for MapIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "map version {}, SHA-256 ", self.version)?;
        for byte in self.sha256 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapIdentityMismatch {
    pub loaded: MapIdentity,
    pub saved: MapIdentity,
}

impl fmt::Display for MapIdentityMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "save uses {}, loaded {}", self.saved, self.loaded)
    }
}

impl std::error::Error for MapIdentityMismatch {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProvinceKind {
    Land,
    Sea,
    Lake,
}

/// Declaration order defines the order of typed edges to the same destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeKind {
    Normal,
    River,
    Strait,
    Port,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProvinceEdge {
    pub to: ProvinceId,
    pub kind: EdgeKind,
    /// Movement cost in whole game ticks.
    pub cost: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Province {
    pub id: ProvinceId,
    pub key: String,
    pub kind: ProvinceKind,
    pub terrain: String,
    pub centroid: [i32; 2],
    pub development: u32,
    pub neighbors: Vec<ProvinceEdge>,
    pub ports: Vec<ProvinceId>,
}

/// An immutable graph whose provinces and connections have a stable iteration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvinceGraph {
    identity: MapIdentity,
    provinces: BTreeMap<ProvinceId, Province>,
    keys: BTreeMap<String, ProvinceId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvinceGraphError(String);

impl fmt::Display for ProvinceGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProvinceGraphError {}

impl ProvinceGraph {
    /// Validates graph structure. The caller supplies the map identity from its file format.
    pub fn try_new(
        identity: MapIdentity,
        provinces: impl IntoIterator<Item = Province>,
    ) -> Result<Self, ProvinceGraphError> {
        let mut by_id = BTreeMap::new();
        let mut keys = BTreeMap::new();

        for mut province in provinces {
            let id = province.id.get();
            if id == 0 {
                return Err(ProvinceGraphError("province ID 0 is reserved".into()));
            }
            if by_id.contains_key(&province.id) {
                return Err(ProvinceGraphError(format!("duplicate province ID {id}")));
            }
            for (field, key) in [("key", &province.key), ("terrain", &province.terrain)] {
                if !is_content_key(key) {
                    return Err(ProvinceGraphError(format!(
                        "province {id} has invalid {field} {key:?}; expected a snake_case key"
                    )));
                }
            }
            if keys.insert(province.key.clone(), province.id).is_some() {
                return Err(ProvinceGraphError(format!(
                    "duplicate province key {:?}",
                    province.key
                )));
            }

            province.neighbors.sort_unstable();
            province.ports.sort_unstable();
            if province.neighbors.iter().any(|edge| edge.cost == 0) {
                return Err(ProvinceGraphError(format!(
                    "province {id} has a zero-cost edge"
                )));
            }
            if province
                .neighbors
                .windows(2)
                .any(|pair| (pair[0].to, pair[0].kind) == (pair[1].to, pair[1].kind))
            {
                return Err(ProvinceGraphError(format!(
                    "province {id} has a duplicate typed edge"
                )));
            }
            if province.ports.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(ProvinceGraphError(format!(
                    "province {id} has a duplicate port"
                )));
            }
            by_id.insert(province.id, province);
        }

        for province in by_id.values() {
            for edge in &province.neighbors {
                let target = by_id.get(&edge.to).ok_or_else(|| {
                    ProvinceGraphError(format!(
                        "province {} has an edge to unknown province {}",
                        province.id.get(),
                        edge.to.get()
                    ))
                })?;
                let reverse = ProvinceEdge {
                    to: province.id,
                    kind: edge.kind,
                    cost: edge.cost,
                };
                if target.neighbors.binary_search(&reverse).is_err() {
                    return Err(ProvinceGraphError(format!(
                        "province {} has no matching reverse {:?} edge and cost to province {}",
                        target.id.get(),
                        edge.kind,
                        province.id.get()
                    )));
                }
            }
            for port in &province.ports {
                match by_id.get(port) {
                    Some(target) if target.kind == ProvinceKind::Sea => {}
                    Some(_) => {
                        return Err(ProvinceGraphError(format!(
                            "province {} has a port to non-sea province {}",
                            province.id.get(),
                            port.get()
                        )));
                    }
                    None => {
                        return Err(ProvinceGraphError(format!(
                            "province {} has a port to unknown province {}",
                            province.id.get(),
                            port.get()
                        )));
                    }
                }
            }
        }

        Ok(Self {
            identity,
            provinces: by_id,
            keys,
        })
    }

    pub const fn identity(&self) -> MapIdentity {
        self.identity
    }

    pub fn get(&self, id: ProvinceId) -> Option<&Province> {
        self.provinces.get(&id)
    }

    pub fn by_key(&self, key: &str) -> Option<&Province> {
        self.keys.get(key).and_then(|id| self.get(*id))
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Province> {
        self.provinces.values()
    }

    /// Returns sorted edges, or `None` for an unknown province.
    pub fn neighbors(&self, id: ProvinceId) -> Option<&[ProvinceEdge]> {
        self.get(id).map(|province| province.neighbors.as_slice())
    }
}

pub(crate) fn is_content_key(key: &str) -> bool {
    key.starts_with(|c: char| c.is_ascii_lowercase())
        && key.split('_').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}
