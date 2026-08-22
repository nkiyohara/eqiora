use super::model::{
    AnalysisCaps, AnalysisFailure, AnalysisHead, CargoAuthorityAnalysis, CargoAuthorityRecord,
    CargoAuthorityRequest, CargoContentRecord, CargoGraphAuthority, CaseId, Completeness,
    ExactOverlay, ExactRepoPath, GitMode, NonEmptySortedSet, OverlayEntry, OverlayStatus,
    RevisionIdentity, RevisionSide, Sha256, SortedSet,
};
use super::repository::{TreeImage, git_blob_oid};

const ZERO_OID: &str = "0000000000000000000000000000000000000000";
const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub(super) fn sha256(bytes: &[u8]) -> Sha256 {
    let bit_len = (bytes.len() as u64).wrapping_mul(8);
    let padded_len = (bytes.len() + 9).div_ceil(64) * 64;
    let mut padded = Vec::with_capacity(padded_len);
    padded.extend_from_slice(bytes);
    padded.push(0x80);
    padded.resize(padded_len - 8, 0);
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = [
        0x6a09e667_u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for block in padded.as_chunks::<64>().0 {
        sha256_block(&mut state, block);
    }
    Sha256(state.iter().map(|word| format!("{word:08x}")).collect())
}

fn sha256_block(state: &mut [u32; 8], block: &[u8]) {
    let mut words = [0_u32; 64];
    for (index, bytes) in block.as_chunks::<4>().0.iter().take(16).enumerate() {
        words[index] = u32::from_be_bytes(bytes.as_slice().try_into().expect("four bytes"));
    }
    for index in 16..64 {
        let s0 = words[index - 15].rotate_right(7)
            ^ words[index - 15].rotate_right(18)
            ^ (words[index - 15] >> 3);
        let s1 = words[index - 2].rotate_right(17)
            ^ words[index - 2].rotate_right(19)
            ^ (words[index - 2] >> 10);
        words[index] = words[index - 16]
            .wrapping_add(s0)
            .wrapping_add(words[index - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for (word, constant) in words.into_iter().zip(SHA256_K) {
        let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choice = (e & f) ^ ((!e) & g);
        let first = h
            .wrapping_add(sum1)
            .wrapping_add(choice)
            .wrapping_add(constant)
            .wrapping_add(word);
        let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let second = sum0.wrapping_add(majority);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(first);
        d = c;
        c = b;
        b = a;
        a = first.wrapping_add(second);
    }
    for (value, addend) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *value = value.wrapping_add(addend);
    }
}

pub(super) fn preflight_added_overlay(
    overlay: &ExactOverlay,
    caps: &AnalysisCaps,
) -> Result<Vec<u8>, AnalysisFailure> {
    let total = overlay_carrier_len(overlay)?;
    if total > caps.max_changed_path_and_raw_diff_bytes {
        return Err(AnalysisFailure::CapBeforeSafeFallback);
    }
    let fact_count = u64::try_from(overlay.entries.0.len())
        .map_err(|_| AnalysisFailure::CapBeforeSafeFallback)?;
    if fact_count > caps.max_changed_facts {
        return Err(AnalysisFailure::CapBeforeSafeFallback);
    }
    let capacity = usize::try_from(total).map_err(|_| AnalysisFailure::CapBeforeSafeFallback)?;
    let mut carrier = Vec::with_capacity(capacity);
    let mut previous = None;
    for entry in &overlay.entries.0 {
        validate_added_entry(entry, previous, caps)?;
        encode_added_entry(entry, &mut carrier)?;
        previous = Some(entry.path.0.as_str());
    }
    if carrier.len() != capacity || sha256(&carrier) != overlay.overlay_sha256 {
        return Err(AnalysisFailure::InvalidOverlay);
    }
    Ok(carrier)
}

fn overlay_carrier_len(overlay: &ExactOverlay) -> Result<u64, AnalysisFailure> {
    if overlay.entries.0.is_empty() {
        return Err(AnalysisFailure::InvalidOverlay);
    }
    overlay.entries.0.iter().try_fold(0_u64, |total, entry| {
        let bytes = entry
            .head_bytes
            .as_ref()
            .ok_or(AnalysisFailure::InvalidOverlay)?;
        let path_len =
            u64::try_from(entry.path.0.len()).map_err(|_| AnalysisFailure::InvalidOverlay)?;
        u32::try_from(path_len).map_err(|_| AnalysisFailure::InvalidOverlay)?;
        let head_len = u64::try_from(bytes.0.len()).map_err(|_| AnalysisFailure::InvalidOverlay)?;
        total
            .checked_add(105)
            .and_then(|value| value.checked_add(path_len))
            .and_then(|value| value.checked_add(head_len))
            .ok_or(AnalysisFailure::CapBeforeSafeFallback)
    })
}

fn validate_added_entry(
    entry: &OverlayEntry,
    previous: Option<&str>,
    caps: &AnalysisCaps,
) -> Result<(), AnalysisFailure> {
    match super::model::validate_repo_path(&entry.path.0, caps.max_repo_path_bytes) {
        Ok(()) => {}
        Err(AnalysisFailure::CapBeforeSafeFallback) => {
            return Err(AnalysisFailure::CapBeforeSafeFallback);
        }
        Err(_) => return Err(AnalysisFailure::InvalidOverlay),
    }
    let bytes = entry
        .head_bytes
        .as_ref()
        .ok_or(AnalysisFailure::InvalidOverlay)?;
    let valid = entry.status == OverlayStatus::Added
        && entry.base_mode == GitMode::Absent
        && entry.base_blob.0 == ZERO_OID
        && entry.head_mode == GitMode::Regular
        && previous.is_none_or(|path| path < entry.path.0.as_str())
        && entry.head_blob == git_blob_oid(&bytes.0);
    valid.then_some(()).ok_or(AnalysisFailure::InvalidOverlay)
}

fn encode_added_entry(entry: &OverlayEntry, carrier: &mut Vec<u8>) -> Result<(), AnalysisFailure> {
    let bytes = entry
        .head_bytes
        .as_ref()
        .ok_or(AnalysisFailure::InvalidOverlay)?;
    let path_len =
        u32::try_from(entry.path.0.len()).map_err(|_| AnalysisFailure::InvalidOverlay)?;
    let head_len = u64::try_from(bytes.0.len()).map_err(|_| AnalysisFailure::InvalidOverlay)?;
    carrier.push(b'A');
    carrier.extend_from_slice(&path_len.to_be_bytes());
    carrier.extend_from_slice(entry.path.0.as_bytes());
    carrier.extend_from_slice(b"000000");
    carrier.extend_from_slice(ZERO_OID.as_bytes());
    carrier.extend_from_slice(b"100644");
    carrier.extend_from_slice(entry.head_blob.0.as_bytes());
    carrier.extend_from_slice(&head_len.to_be_bytes());
    carrier.extend_from_slice(&bytes.0);
    Ok(())
}

pub(super) fn apply_added_overlay(
    base: &TreeImage,
    overlay: &ExactOverlay,
    carrier: &[u8],
    caps: &AnalysisCaps,
) -> Result<TreeImage, AnalysisFailure> {
    if carrier.len() as u64 > caps.max_changed_path_and_raw_diff_bytes
        || sha256(carrier) != overlay.overlay_sha256
    {
        return Err(AnalysisFailure::ConcurrentOverlayChange);
    }
    if overlay
        .entries
        .0
        .iter()
        .any(|entry| base.contains(&entry.path.0))
    {
        return Err(AnalysisFailure::InvalidOverlay);
    }
    let mut head = base.clone();
    for entry in &overlay.entries.0 {
        let bytes = entry
            .head_bytes
            .as_ref()
            .ok_or(AnalysisFailure::ConcurrentOverlayChange)?;
        head.added_regular(
            entry.path.0.clone(),
            entry.head_blob.clone(),
            bytes.0.clone(),
        )?;
    }
    Ok(head)
}

pub(super) fn finish_analysis(
    request: &CargoAuthorityRequest,
    base: RevisionIdentity,
    head: RevisionIdentity,
    graphs: NonEmptySortedSet<CargoGraphAuthority>,
) -> Result<CargoAuthorityAnalysis, AnalysisFailure> {
    if let AnalysisHead::Overlay(overlay) = &request.head {
        validate_changed_manifests(overlay, &graphs)?;
    }
    Ok(CargoAuthorityAnalysis {
        base,
        head,
        graphs,
        completeness: Completeness::Complete,
        precise_cases: SortedSet::<CaseId>::new(),
        unknowns: SortedSet::<ExactRepoPath>::new(),
    })
}

fn validate_changed_manifests(
    overlay: &ExactOverlay,
    graphs: &NonEmptySortedSet<CargoGraphAuthority>,
) -> Result<(), AnalysisFailure> {
    for entry in overlay
        .entries
        .0
        .iter()
        .filter(|entry| entry.path.0.ends_with("/Cargo.toml"))
    {
        for graph in graphs
            .0
            .iter()
            .filter(|graph| graph.revision.side == RevisionSide::Head)
        {
            let matches: Vec<_> = graph
                .records
                .iter()
                .filter_map(|record| match record {
                    CargoAuthorityRecord::Manifest(content) if content.path == entry.path => {
                        Some(content)
                    }
                    _ => None,
                })
                .collect();
            if matches.is_empty() {
                return Err(AnalysisFailure::RequiredCoverageMissing);
            }
            let expected = expected_manifest(entry, graph)?;
            if matches.len() != 1 || matches[0] != &expected {
                return Err(AnalysisFailure::AuthorityConflict);
            }
        }
    }
    Ok(())
}

fn expected_manifest(
    entry: &OverlayEntry,
    graph: &CargoGraphAuthority,
) -> Result<CargoContentRecord, AnalysisFailure> {
    let bytes = entry
        .head_bytes
        .as_ref()
        .ok_or(AnalysisFailure::InternalFailure)?;
    Ok(CargoContentRecord {
        revision: graph.revision.clone(),
        path: entry.path.clone(),
        mode: entry.head_mode.clone(),
        blob: entry.head_blob.clone(),
        byte_len: bytes.0.len() as u64,
        content_sha256: sha256(&bytes.0),
    })
}
