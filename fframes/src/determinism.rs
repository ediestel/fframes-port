use std::collections::BTreeMap;
use std::path::Path;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    pub fn from_bytes(input: &[u8]) -> Self {
        let mut out = [0_u8; 32];
        let mut state = [FNV_OFFSET; 3];
        for (i, chunk) in input.chunks(4096).enumerate() {
            let mix = fnv1a(chunk).rotate_left(((i * 7) % 64) as u32);
            state[i % 3] = state[i % 3].wrapping_mul(FNV_PRIME) ^ mix;
        }
        let final_hash = fnv1a(input);
        for (i, b) in final_hash.to_le_bytes().iter().enumerate() {
            out[i] = *b;
        }
        for (i, s) in state.iter().enumerate() {
            for (j, b) in s.to_le_bytes().iter().enumerate() {
                out[8 + i * 8 + j] = *b;
            }
        }
        ContentHash(out)
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

#[derive(Clone, Debug)]
pub struct InputFingerprint {
    fields: BTreeMap<String, Vec<u8>>,
}

impl InputFingerprint {
    pub fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    pub fn add(mut self, key: impl Into<String>, value: impl AsRef<[u8]>) -> Self {
        self.fields.insert(key.into(), value.as_ref().to_vec());
        self
    }

    pub fn add_u32(self, key: impl Into<String>, value: u32) -> Self {
        self.add(key, value.to_le_bytes())
    }

    pub fn add_u64(self, key: impl Into<String>, value: u64) -> Self {
        self.add(key, value.to_le_bytes())
    }

    pub fn add_str(self, key: impl Into<String>, value: &str) -> Self {
        self.add(key, value.as_bytes())
    }

    pub fn finalize(&self) -> ContentHash {
        let mut out = Vec::with_capacity(4096);
        for (k, v) in &self.fields {
            out.extend_from_slice(k.as_bytes());
            out.push(0);
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            out.extend_from_slice(v);
        }
        ContentHash::from_bytes(&out)
    }

    pub fn merge(mut self, other: InputFingerprint) -> Self {
        for (k, v) in other.fields {
            self.fields.insert(k, v);
        }
        self
    }
}

impl Default for InputFingerprint {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct OutputHash {
    pub hash: ContentHash,
    pub byte_count: u64,
}

impl OutputHash {
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self {
            hash: ContentHash::from_bytes(bytes),
            byte_count: bytes.len() as u64,
        }
    }

    pub fn of_file(path: &Path) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        Ok(Self::of_bytes(&bytes))
    }
}

#[derive(Clone, Debug)]
pub struct RenderResult {
    pub output_hash: OutputHash,
    pub input_fingerprint: ContentHash,
    pub frame_count: u64,
}

#[derive(Debug)]
pub enum DeterminismError {
    InputMismatch {
        left: ContentHash,
        right: ContentHash,
    },
    OutputMismatch {
        left: ContentHash,
        right: ContentHash,
    },
    FrameCountMismatch {
        left: u64,
        right: u64,
    },
}

impl std::fmt::Display for DeterminismError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputMismatch { left, right } => {
                write!(f, "input fingerprint mismatch: {left} vs {right}")
            }
            Self::OutputMismatch { left, right } => {
                write!(f, "output hash mismatch: {left} vs {right}")
            }
            Self::FrameCountMismatch { left, right } => {
                write!(f, "frame count mismatch: {left} vs {right}")
            }
        }
    }
}

impl std::error::Error for DeterminismError {}

pub fn assert_deterministic(
    left: &RenderResult,
    right: &RenderResult,
) -> Result<(), DeterminismError> {
    if left.input_fingerprint != right.input_fingerprint {
        return Err(DeterminismError::InputMismatch {
            left: left.input_fingerprint.clone(),
            right: right.input_fingerprint.clone(),
        });
    }
    if left.frame_count != right.frame_count {
        return Err(DeterminismError::FrameCountMismatch {
            left: left.frame_count,
            right: right.frame_count,
        });
    }
    if left.output_hash.hash != right.output_hash.hash {
        return Err(DeterminismError::OutputMismatch {
            left: left.output_hash.hash.clone(),
            right: right.output_hash.hash.clone(),
        });
    }
    Ok(())
}

pub fn deterministic_session_id(fingerprint: &ContentHash) -> String {
    let hex = fingerprint.to_hex();
    format!("fframes-{}", &hex[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable() {
        let a = ContentHash::from_bytes(b"hello");
        let b = ContentHash::from_bytes(b"hello");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_differs() {
        let a = ContentHash::from_bytes(b"hello");
        let b = ContentHash::from_bytes(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn fingerprint_is_order_independent() {
        let a = InputFingerprint::new()
            .add_u32("width", 1920)
            .add_u32("height", 1080);
        let b = InputFingerprint::new()
            .add_u32("height", 1080)
            .add_u32("width", 1920);
        assert_eq!(a.finalize(), b.finalize());
    }

    #[test]
    fn session_is_deterministic() {
        let fp = InputFingerprint::new().add_u32("w", 1).finalize();
        let id1 = deterministic_session_id(&fp);
        let id2 = deterministic_session_id(&fp);
        assert_eq!(id1, id2);
    }
}
