/// Trust System — 5-tier trust for mesh peers.
///
/// Trust is earned by the Apis instance itself, never granted by users.
/// Users cannot promote, demote, whitelist, or influence trust in any way.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::network::messages::PeerId;

/// Trust levels — higher = more privileges on the mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TrustLevel {
    /// Failed binary attestation or unknown hash. Silently dropped.
    Unattested = 0,
    /// Passed attestation. Can receive data but cannot send.
    Attested = 1,
    /// 48h uptime + consistent valid messages. Can share lessons + synaptic.
    Verified = 2,
    /// 7d sustained presence + zero violations. Can share golden + weights.
    Trusted = 3,
    /// Explicit peer-key in ATTESTATION.json. Can share code patches + weight transfers.
    Core = 4,
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustLevel::Unattested => write!(f, "Unattested"),
            TrustLevel::Attested => write!(f, "Attested"),
            TrustLevel::Verified => write!(f, "Verified"),
            TrustLevel::Trusted => write!(f, "Trusted"),
            TrustLevel::Core => write!(f, "Core"),
        }
    }
}

/// Per-peer trust record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerTrust {
    pub peer_id: PeerId,
    pub level: TrustLevel,
    pub first_seen: String,          // RFC3339
    pub last_seen: String,           // RFC3339
    pub valid_messages: u64,         // Count of schema-valid signed messages received
    pub violations: u32,             // Lifetime violation count
    pub last_violation: Option<String>, // RFC3339
    pub attestation_verified: bool,  // Has passed challenge-response attestation
    pub binary_hash: Option<String>, // Last known binary hash
}

impl PeerTrust {
    pub fn new_unattested(peer_id: PeerId) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            peer_id,
            level: TrustLevel::Unattested,
            first_seen: now.clone(),
            last_seen: now,
            valid_messages: 0,
            violations: 0,
            last_violation: None,
            attestation_verified: false,
            binary_hash: None,
        }
    }

    /// Record a valid message from this peer. May promote trust level.
    pub fn record_valid_message(&mut self) {
        self.valid_messages += 1;
        self.last_seen = chrono::Utc::now().to_rfc3339();
        self.evaluate_promotion();
    }

    /// Record a successful attestation.
    pub fn record_attestation(&mut self, binary_hash: &str) {
        self.attestation_verified = true;
        self.binary_hash = Some(binary_hash.to_string());
        if self.level < TrustLevel::Attested {
            self.level = TrustLevel::Attested;
            tracing::info!("[TRUST] ⬆️ Peer {} promoted to Attested", self.peer_id);
        }
        self.evaluate_promotion();
    }

    /// Evaluate whether this peer should be promoted based on track record.
    fn evaluate_promotion(&mut self) {
        if !self.attestation_verified {
            return; // Can't promote without attestation
        }

        let now = chrono::Utc::now();
        let first = chrono::DateTime::parse_from_rfc3339(&self.first_seen)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or(now);
        let uptime = now - first;

        // Attested → Verified: 48h uptime + 10 valid messages + no recent violations
        if self.level == TrustLevel::Attested
            && uptime >= chrono::Duration::hours(48)
            && self.valid_messages >= 10
            && self.violations == 0
        {
            self.level = TrustLevel::Verified;
            tracing::info!("[TRUST] ⬆️ Peer {} promoted to Verified ({}h uptime, {} valid msgs)",
                self.peer_id, uptime.num_hours(), self.valid_messages);
        }

        // Verified → Trusted: 7d sustained + 100 valid messages + zero violations
        if self.level == TrustLevel::Verified
            && uptime >= chrono::Duration::days(7)
            && self.valid_messages >= 100
            && self.violations == 0
        {
            self.level = TrustLevel::Trusted;
            tracing::info!("[TRUST] ⬆️ Peer {} promoted to Trusted ({}d uptime, {} valid msgs)",
                self.peer_id, uptime.num_days(), self.valid_messages);
        }

        // Core is NEVER auto-promoted — must be in ATTESTATION.json
    }

    /// Record a violation. May demote trust level.
    pub fn record_violation(&mut self) {
        self.violations += 1;
        self.last_violation = Some(chrono::Utc::now().to_rfc3339());

        // Any violation demotes to Attested (can't go lower without quarantine)
        if self.level > TrustLevel::Attested {
            tracing::warn!("[TRUST] ⬇️ Peer {} demoted from {} to Attested (violation #{})",
                self.peer_id, self.level, self.violations);
            self.level = TrustLevel::Attested;
        }
    }
}

/// Trust store — manages trust levels for all known peers.
pub struct TrustStore {
    peers: HashMap<PeerId, PeerTrust>,
    /// Core peer IDs loaded from ATTESTATION.json
    core_peers: Vec<PeerId>,
    persist_path: std::path::PathBuf,
}

impl TrustStore {
    pub fn new(persist_dir: &std::path::Path) -> Self {
        let persist_path = persist_dir.join("trust.json");
        let _ = std::fs::create_dir_all(persist_dir);

        let peers = if persist_path.exists() {
            std::fs::read_to_string(&persist_path)
                .ok()
                .and_then(|s| serde_json::from_str::<HashMap<PeerId, PeerTrust>>(&s).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        // Load core peer IDs from ATTESTATION.json if it exists
        let core_peers = Self::load_core_peers();

        Self { peers, core_peers, persist_path }
    }

    fn load_core_peers() -> Vec<PeerId> {
        let path = std::path::Path::new("ATTESTATION.json");
        if !path.exists() {
            return vec![];
        }

        #[derive(Deserialize)]
        struct AttestationFile {
            #[serde(default)]
            core_peers: Vec<String>,
        }

        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<AttestationFile>(&s).ok())
            .map(|af| af.core_peers.into_iter().map(PeerId).collect())
            .unwrap_or_default()
    }

    /// Get or create trust record for a peer.
    pub fn get_or_create(&mut self, peer_id: &PeerId) -> &mut PeerTrust {
        if !self.peers.contains_key(peer_id) {
            let mut trust = PeerTrust::new_unattested(peer_id.clone());
            // Check if this peer is in the Core allowlist
            if self.core_peers.contains(peer_id) {
                trust.level = TrustLevel::Core;
                trust.attestation_verified = true;
                tracing::info!("[TRUST] 👑 Peer {} is a Core peer (in ATTESTATION.json)", peer_id);
            }
            self.peers.insert(peer_id.clone(), trust);
        }
        self.peers.get_mut(peer_id).unwrap()
    }

    /// Get trust level for a peer.
    pub fn trust_level(&self, peer_id: &PeerId) -> TrustLevel {
        self.peers.get(peer_id)
            .map(|t| t.level)
            .unwrap_or(TrustLevel::Unattested)
    }

    /// Check if a peer can share a specific kind of data.
    pub fn can_share_lessons(&self, peer_id: &PeerId) -> bool {
        self.trust_level(peer_id) >= TrustLevel::Verified
    }

    pub fn can_share_golden(&self, peer_id: &PeerId) -> bool {
        self.trust_level(peer_id) >= TrustLevel::Trusted
    }

    pub fn can_share_weights(&self, peer_id: &PeerId) -> bool {
        self.trust_level(peer_id) >= TrustLevel::Core
    }

    pub fn can_share_code(&self, peer_id: &PeerId) -> bool {
        self.trust_level(peer_id) >= TrustLevel::Core
    }

    /// Persist trust state to disk.
    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.peers) {
            let _ = std::fs::write(&self.persist_path, json);
        }
    }

    /// Get all known peers with their trust levels.
    pub fn all_peers(&self) -> Vec<&PeerTrust> {
        self.peers.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_peer() -> PeerId {
        PeerId("test_peer_abc123def456".to_string())
    }

    #[test]
    fn test_new_peer_is_unattested() {
        let trust = PeerTrust::new_unattested(test_peer());
        assert_eq!(trust.level, TrustLevel::Unattested);
        assert!(!trust.attestation_verified);
    }

    #[test]
    fn test_attestation_promotes_to_attested() {
        let mut trust = PeerTrust::new_unattested(test_peer());
        trust.record_attestation("abc123hash");
        assert_eq!(trust.level, TrustLevel::Attested);
        assert!(trust.attestation_verified);
    }

    #[test]
    fn test_violation_demotes() {
        let mut trust = PeerTrust::new_unattested(test_peer());
        trust.record_attestation("abc123hash");
        trust.level = TrustLevel::Trusted; // Manually set for test
        trust.record_violation();
        assert_eq!(trust.level, TrustLevel::Attested); // Demoted
        assert_eq!(trust.violations, 1);
    }

    #[test]
    fn test_trust_level_ordering() {
        assert!(TrustLevel::Core > TrustLevel::Trusted);
        assert!(TrustLevel::Trusted > TrustLevel::Verified);
        assert!(TrustLevel::Verified > TrustLevel::Attested);
        assert!(TrustLevel::Attested > TrustLevel::Unattested);
    }

    #[test]
    fn test_trust_store_core_peers() {
        let tmp = std::env::temp_dir().join(format!("hive_trust_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);

        let mut store = TrustStore::new(&tmp);
        let peer = test_peer();
        let trust = store.get_or_create(&peer);
        assert_eq!(trust.level, TrustLevel::Unattested);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn test_permission_checks() {
        let tmp = std::env::temp_dir().join(format!("hive_trust_perm_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);

        let mut store = TrustStore::new(&tmp);
        let peer = test_peer();
        store.get_or_create(&peer).record_attestation("hash");

        assert!(!store.can_share_lessons(&peer)); // Attested can't share
        assert!(!store.can_share_code(&peer));    // Only Core can

        store.get_or_create(&peer).level = TrustLevel::Verified;
        assert!(store.can_share_lessons(&peer));  // Verified can share lessons
        assert!(!store.can_share_golden(&peer));  // But not golden

        store.get_or_create(&peer).level = TrustLevel::Core;
        assert!(store.can_share_code(&peer));     // Core can share everything

        std::fs::remove_dir_all(&tmp).ok();
    }
}
