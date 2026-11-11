use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Upper bound on a canonical state before transport framing.
pub const MAX_STATE_BYTES: usize = crate::MAX_SERVER_FRAME_BYTES - 64;

/// A splice with an offset and removal length measured in the base snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ByteEdit {
    pub offset: u32,
    pub remove: u32,
    pub insert: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StateDelta {
    pub base_sequence: u64,
    pub sequence: u64,
    pub edits: Vec<ByteEdit>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplicaError {
    TooLarge,
    SequenceOverflow,
    InvalidSequence {
        expected_base: u64,
        actual_base: u64,
        actual_sequence: u64,
    },
    InvalidEdit {
        index: usize,
    },
}

impl std::fmt::Display for ReplicaError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => write!(formatter, "LAN state or delta exceeds the size limit"),
            Self::SequenceOverflow => write!(formatter, "LAN state sequence exhausted"),
            Self::InvalidSequence {
                expected_base,
                actual_base,
                actual_sequence,
            } => write!(
                formatter,
                "expected a delta after {expected_base}, got {actual_base} → {actual_sequence}",
            ),
            Self::InvalidEdit { index } => {
                write!(formatter, "invalid LAN state delta edit {index}")
            }
        }
    }
}

impl std::error::Error for ReplicaError {}

#[derive(Debug)]
pub struct Replica {
    sequence: u64,
    bytes: Vec<u8>,
}

impl Replica {
    pub fn new(sequence: u64, bytes: Vec<u8>) -> Result<Self, ReplicaError> {
        check_size(bytes.len())?;
        Ok(Self { sequence, bytes })
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn apply(&mut self, delta: &StateDelta) -> Result<(), ReplicaError> {
        let expected_sequence = self
            .sequence
            .checked_add(1)
            .ok_or(ReplicaError::SequenceOverflow)?;
        if delta.base_sequence != self.sequence || delta.sequence != expected_sequence {
            return Err(ReplicaError::InvalidSequence {
                expected_base: self.sequence,
                actual_base: delta.base_sequence,
                actual_sequence: delta.sequence,
            });
        }

        let mut previous_offset = None;
        let mut previous_end = 0;
        let mut removed = 0;
        let mut inserted = 0;
        let mut edit_size = DELTA_HEADER_BYTES;
        for (index, edit) in delta.edits.iter().enumerate() {
            let offset = edit.offset as usize;
            let remove = edit.remove as usize;
            let end = offset
                .checked_add(remove)
                .ok_or(ReplicaError::InvalidEdit { index })?;
            if offset < previous_end
                || previous_offset.is_some_and(|previous| offset <= previous)
                || end > self.bytes.len()
                || (remove == 0 && edit.insert.is_empty())
            {
                return Err(ReplicaError::InvalidEdit { index });
            }
            edit_size = add_edit_size(edit_size, edit.insert.len())?;
            previous_offset = Some(offset);
            previous_end = end;
            removed += remove;
            inserted += edit.insert.len();
        }
        let length = self.bytes.len() - removed + inserted;
        check_size(length)?;

        if !delta.edits.is_empty() {
            let mut bytes = Vec::with_capacity(length);
            let mut cursor = 0;
            for edit in &delta.edits {
                let offset = edit.offset as usize;
                bytes.extend_from_slice(&self.bytes[cursor..offset]);
                bytes.extend_from_slice(&edit.insert);
                cursor = offset + edit.remove as usize;
            }
            bytes.extend_from_slice(&self.bytes[cursor..]);
            self.bytes = bytes;
        }
        self.sequence = expected_sequence;
        Ok(())
    }
}

pub fn make_delta(base_sequence: u64, old: &[u8], new: &[u8]) -> Result<StateDelta, ReplicaError> {
    check_size(old.len())?;
    check_size(new.len())?;
    let sequence = base_sequence
        .checked_add(1)
        .ok_or(ReplicaError::SequenceOverflow)?;
    let mut edits = Vec::new();
    let mut edit_size = DELTA_HEADER_BYTES;
    if old.len() == new.len() {
        let mut start = None;
        for (offset, (before, after)) in old.iter().zip(new).enumerate() {
            if before == after {
                if let Some(start) = start.take() {
                    edit_size = add_edit_size(edit_size, offset - start)?;
                    edits.push(replacement(start, offset - start, &new[start..offset]));
                }
            } else {
                start.get_or_insert(offset);
            }
        }
        if let Some(start) = start {
            add_edit_size(edit_size, new.len() - start)?;
            edits.push(replacement(start, old.len() - start, &new[start..]));
        }
    } else {
        let prefix = old
            .iter()
            .zip(new)
            .take_while(|(before, after)| before == after)
            .count();
        let suffix = old[prefix..]
            .iter()
            .rev()
            .zip(new[prefix..].iter().rev())
            .take_while(|(before, after)| before == after)
            .count();
        let insert = &new[prefix..new.len() - suffix];
        add_edit_size(edit_size, insert.len())?;
        edits.push(replacement(prefix, old.len() - prefix - suffix, insert));
    }
    Ok(StateDelta {
        base_sequence,
        sequence,
        edits,
    })
}

pub fn canonical_checksum(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"strategy:lan-state:v1\0");
    hash.update(bytes);
    hash.finalize().into()
}

fn replacement(offset: usize, remove: usize, insert: &[u8]) -> ByteEdit {
    ByteEdit {
        offset: offset as u32,
        remove: remove as u32,
        insert: insert.to_vec(),
    }
}

// Conservative upper bounds for postcard's two u64 sequences and edit count,
// and each edit's two u32 coordinates and length prefix.
const DELTA_HEADER_BYTES: usize = 24;
const EDIT_HEADER_BYTES: usize = 16;

fn add_edit_size(current: usize, insert: usize) -> Result<usize, ReplicaError> {
    let size = current
        .checked_add(EDIT_HEADER_BYTES)
        .and_then(|size| size.checked_add(insert))
        .ok_or(ReplicaError::TooLarge)?;
    check_size(size)?;
    Ok(size)
}

fn check_size(size: usize) -> Result<(), ReplicaError> {
    if size > MAX_STATE_BYTES {
        return Err(ReplicaError::TooLarge);
    }
    Ok(())
}
