use sha2::{Digest as ShaDigest, Sha256};
use uuid::Uuid;

use anni_ingest::{Digest, SafeRelativePath};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputReceipt {
    path: SafeRelativePath,
    byte_length: u64,
    digest: Digest,
}

impl OutputReceipt {
    pub const fn new(path: SafeRelativePath, byte_length: u64, digest: Digest) -> Self {
        Self {
            path,
            byte_length,
            digest,
        }
    }

    pub const fn path(&self) -> &SafeRelativePath {
        &self.path
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub const fn digest(&self) -> Digest {
        self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReceipt {
    job_id: Uuid,
    manifest_digest: Digest,
    plan_digest: Digest,
    outputs: Vec<OutputReceipt>,
    digest: Digest,
}

impl ExecutionReceipt {
    pub(crate) fn new(
        job_id: Uuid,
        manifest_digest: Digest,
        plan_digest: Digest,
        mut outputs: Vec<OutputReceipt>,
    ) -> Self {
        outputs.sort_by(|left, right| left.path.cmp(&right.path));
        let digest = receipt_digest(job_id, manifest_digest, plan_digest, &outputs);
        Self {
            job_id,
            manifest_digest,
            plan_digest,
            outputs,
            digest,
        }
    }

    pub const fn job_id(&self) -> Uuid {
        self.job_id
    }

    pub const fn manifest_digest(&self) -> Digest {
        self.manifest_digest
    }

    pub const fn plan_digest(&self) -> Digest {
        self.plan_digest
    }

    pub fn outputs(&self) -> &[OutputReceipt] {
        &self.outputs
    }

    pub const fn digest(&self) -> Digest {
        self.digest
    }
}

fn receipt_digest(
    job_id: Uuid,
    manifest_digest: Digest,
    plan_digest: Digest,
    outputs: &[OutputReceipt],
) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(b"anni-ingest-execution-receipt-v1\0");
    hasher.update(job_id.as_bytes());
    hasher.update(manifest_digest.as_bytes());
    hasher.update(plan_digest.as_bytes());
    hasher.update((outputs.len() as u64).to_be_bytes());
    for output in outputs {
        hasher.update((output.path.as_str().len() as u64).to_be_bytes());
        hasher.update(output.path.as_str().as_bytes());
        hasher.update(output.byte_length.to_be_bytes());
        hasher.update(output.digest.as_bytes());
    }
    Digest::new(hasher.finalize().into())
}
