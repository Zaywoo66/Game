use std::{fmt, fs, num::NonZeroU32, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use strategy_sim::{
    EdgeKind, MapIdentity, Province, ProvinceEdge, ProvinceGraph, ProvinceGraphError, ProvinceId,
    ProvinceKind,
};

use crate::{deserialize_present, is_content_key};

const LEGACY_VERSION: u32 = 1;
const WEIGHTED_VERSION: u32 = 2;
const HIERARCHY_VERSION: u32 = 3;

#[derive(Debug)]
pub enum ProvinceLoadError {
    Io(std::io::Error),
    Json(serde_json::Error),
    UnsupportedSchemaVersion(u32),
    EdgeCostVersionMismatch {
        version: u32,
        province: u32,
        target: u32,
    },
    ProjectionVersionMismatch(u32),
    ProvinceMetadataVersionMismatch {
        version: u32,
        province: u32,
    },
    InvalidProvinceMetadataKey {
        province: u32,
        field: &'static str,
        key: String,
    },
    InvalidMapHash,
    MapHashMismatch {
        declared: MapIdentity,
        computed: MapIdentity,
    },
    Graph(ProvinceGraphError),
}

impl fmt::Display for ProvinceLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "could not read map: {error}"),
            Self::Json(error) => write!(f, "invalid province JSON: {error}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(f, "unsupported province schema_version {version}")
            }
            Self::EdgeCostVersionMismatch {
                version,
                province,
                target,
            } => {
                write!(f, "province schema v{version} has the wrong cost field for edge {province} -> {target}")
            }
            Self::ProjectionVersionMismatch(version) => {
                write!(
                    f,
                    "province schema v{version} has the wrong projection field"
                )
            }
            Self::ProvinceMetadataVersionMismatch { version, province } => {
                write!(f, "province schema v{version} has the wrong name_key or area field for province {province}")
            }
            Self::InvalidProvinceMetadataKey {
                province,
                field,
                key,
            } => {
                write!(f, "province {province} has an invalid {field} key {key:?}")
            }
            Self::InvalidMapHash => f.write_str("map_hash must be 64 lowercase hexadecimal digits"),
            Self::MapHashMismatch { declared, computed } => {
                write!(f, "map declares {declared}, computed {computed}")
            }
            Self::Graph(error) => write!(f, "invalid province graph: {error}"),
        }
    }
}

impl std::error::Error for ProvinceLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Graph(error) => Some(error),
            _ => None,
        }
    }
}

/// Reads and validates a versioned province file.
pub fn load_provinces(path: impl AsRef<Path>) -> Result<ProvinceGraph, ProvinceLoadError> {
    let bytes = fs::read(path).map_err(ProvinceLoadError::Io)?;
    parse_provinces(&bytes)
}

/// Validates the file structure, cross-references and canonical hash.
pub fn parse_provinces(bytes: &[u8]) -> Result<ProvinceGraph, ProvinceLoadError> {
    let mut map: MapFile = serde_json::from_slice(bytes).map_err(ProvinceLoadError::Json)?;
    if !matches!(
        map.schema_version,
        LEGACY_VERSION | WEIGHTED_VERSION | HIERARCHY_VERSION
    ) {
        return Err(ProvinceLoadError::UnsupportedSchemaVersion(
            map.schema_version,
        ));
    }

    if map.projection.is_some() != (map.schema_version == HIERARCHY_VERSION) {
        return Err(ProvinceLoadError::ProjectionVersionMismatch(
            map.schema_version,
        ));
    }

    let declared = MapIdentity {
        version: map.map_version,
        sha256: decode_hash(&map.map_hash).ok_or(ProvinceLoadError::InvalidMapHash)?,
    };

    map.provinces.sort_unstable_by_key(|province| province.id);
    for province in &mut map.provinces {
        province.validate_metadata(map.schema_version)?;
        for edge in &province.neighbors {
            if edge.cost.is_some() != (map.schema_version >= WEIGHTED_VERSION) {
                return Err(ProvinceLoadError::EdgeCostVersionMismatch {
                    version: map.schema_version,
                    province: province.id,
                    target: edge.id,
                });
            }
        }
        province.neighbors.sort_unstable();
        province.ports.sort_unstable();
    }
    let canonical = match &map.projection {
        Some(projection) => serde_json::to_vec(&CanonicalHierarchy {
            projection,
            provinces: &map.provinces,
        }),
        None => serde_json::to_vec(&map.provinces),
    }
    .map_err(ProvinceLoadError::Json)?;
    let computed = MapIdentity {
        version: map.map_version,
        sha256: Sha256::digest(canonical).into(),
    };

    let graph = ProvinceGraph::try_new(declared, map.provinces.into_iter().map(Province::from))
        .map_err(ProvinceLoadError::Graph)?;
    if declared != computed {
        return Err(ProvinceLoadError::MapHashMismatch { declared, computed });
    }
    Ok(graph)
}

pub(crate) fn decode_hash(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let mut digest = [0; 32];
    for (i, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(digest)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapFile {
    schema_version: u32,
    map_version: NonZeroU32,
    map_hash: String,
    #[serde(default, deserialize_with = "deserialize_present")]
    projection: Option<ProjectionFile>,
    provinces: Vec<ProvinceFile>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectionFile {
    pixels_per_degree: NonZeroU32,
    origin: [i32; 2],
}

#[derive(Serialize)]
struct CanonicalHierarchy<'a> {
    projection: &'a ProjectionFile,
    provinces: &'a [ProvinceFile],
}

// Field declaration order is part of the canonical province JSON encoding.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProvinceFile {
    id: u32,
    key: String,
    #[serde(with = "ProvinceKindDef")]
    kind: ProvinceKind,
    terrain: String,
    centroid: [i32; 2],
    neighbors: Vec<EdgeFile>,
    ports: Vec<u32>,
    development: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    name_key: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    area: Option<String>,
}

impl ProvinceFile {
    fn validate_metadata(&self, version: u32) -> Result<(), ProvinceLoadError> {
        let fields = match (version, &self.name_key, &self.area) {
            (HIERARCHY_VERSION, Some(name_key), Some(area)) => {
                [("name_key", name_key), ("area", area)]
            }
            (LEGACY_VERSION | WEIGHTED_VERSION, None, None) => return Ok(()),
            _ => {
                return Err(ProvinceLoadError::ProvinceMetadataVersionMismatch {
                    version,
                    province: self.id,
                })
            }
        };
        for (field, key) in fields {
            if !is_content_key(key) {
                return Err(ProvinceLoadError::InvalidProvinceMetadataKey {
                    province: self.id,
                    field,
                    key: key.clone(),
                });
            }
        }
        Ok(())
    }
}

impl From<ProvinceFile> for Province {
    fn from(value: ProvinceFile) -> Self {
        Self {
            id: ProvinceId::new(value.id),
            key: value.key,
            kind: value.kind,
            terrain: value.terrain,
            centroid: value.centroid,
            neighbors: value
                .neighbors
                .into_iter()
                .map(|edge| ProvinceEdge {
                    to: ProvinceId::new(edge.id),
                    kind: edge.edge,
                    cost: edge.cost.unwrap_or(NonZeroU32::MIN).get(),
                })
                .collect(),
            ports: value.ports.into_iter().map(ProvinceId::new).collect(),
            development: value.development,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "ProvinceKind", rename_all = "snake_case")]
enum ProvinceKindDef {
    Land,
    Sea,
    Lake,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct EdgeFile {
    id: u32,
    #[serde(with = "EdgeKindDef")]
    edge: EdgeKind,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    cost: Option<NonZeroU32>,
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "EdgeKind", rename_all = "snake_case")]
pub(super) enum EdgeKindDef {
    Normal,
    River,
    Strait,
    Port,
}
