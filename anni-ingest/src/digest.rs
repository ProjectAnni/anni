use std::fmt;

/// A content digest used to bind manifests, plans, and verification receipts.
///
/// The workflow only needs a fixed-width identity here. Which hashing
/// algorithm produced the bytes belongs to the protocol envelope and can
/// evolve without weakening equality checks inside the state machine.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Digest([u8; 32]);

impl Digest {
    pub const LENGTH: usize = 32;

    pub const fn new(bytes: [u8; Self::LENGTH]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; Self::LENGTH] {
        &self.0
    }
}

impl From<[u8; Digest::LENGTH]> for Digest {
    fn from(value: [u8; Digest::LENGTH]) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Digest({self})")
    }
}
