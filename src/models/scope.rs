use serde::{Deserialize, Serialize};

/// Defines the security scope of an event or memory context.
/// 
/// A primary tenet of the HIVE system is strict data segregation.
/// - `Public`: Broadly accessible data (e.g. general Discord Channels), siloed strictly by `channel_id` + `user_id`.
/// - `Private`: Data tied exclusively to a specific User's identity (e.g. Direct Messages).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scope {
    Public {
        channel_id: String,
        user_id: String,
    },
    /// A secure 1-to-1 context for a specific user ID.
    Private {
        user_id: String,
    },
}

impl Scope {
    /// Determines if a process operating under `self`'s scope is permitted
    /// to read data that is tagged with the `target` scope.
    ///
    /// Rules:
    /// - `Public(C, U)` can read `Public(C, U)`.
    /// - `Private(U)` can read `Private(U)`.
    /// - Mismatched `Public` scopes cannot read each other (Context Siloing).
    /// - `Private(X)` CANNOT read `Private(Y)` data.
    pub fn can_read(&self, target: &Scope) -> bool {
        match (self, target) {
            // Memory Silos: Explicitly require exact channel and user match for public scopes
            (Scope::Public { channel_id: req_c, user_id: req_u }, Scope::Public { channel_id: targ_c, user_id: targ_u }) => {
                req_c == targ_c && req_u == targ_u
            }
            // Cannot cross public/private boundaries contextually
            (Scope::Public { .. }, Scope::Private { .. }) => false,
            (Scope::Private { .. }, Scope::Public { .. }) => false,
            // Private scope can only read its own private data
            (Scope::Private { user_id: req_id }, Scope::Private { user_id: target_id }) => {
                req_id == target_id
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_visibility() {
        let pub_alice_c1 = Scope::Public { channel_id: "c1".to_string(), user_id: "alice".to_string() };
        let pub_bob_c1 = Scope::Public { channel_id: "c1".to_string(), user_id: "bob".to_string() };
        let pub_alice_c2 = Scope::Public { channel_id: "c2".to_string(), user_id: "alice".to_string() };
        
        let priv_alice = Scope::Private { user_id: "alice".to_string() };
        let priv_bob = Scope::Private { user_id: "bob".to_string() };

        // Public match
        assert!(pub_alice_c1.can_read(&pub_alice_c1));
        
        // Public siloing blocks same channel, different user
        assert!(!pub_alice_c1.can_read(&pub_bob_c1));

        // Public siloing blocks same user, different channel
        assert!(!pub_alice_c1.can_read(&pub_alice_c2));
        
        // Public cannot read Private
        assert!(!pub_alice_c1.can_read(&priv_alice));

        // Private cannot read Public
        assert!(!priv_alice.can_read(&pub_alice_c1));

        // Private can read own Private
        assert!(priv_alice.can_read(&priv_alice));

        // Private CANNOT read other's Private
        assert!(!priv_alice.can_read(&priv_bob));
    }

    #[test]
    fn test_scope_serde_and_derives() {
        let pub_scope = Scope::Public { channel_id: "c1".to_string(), user_id: "u1".to_string() };
        let priv_scope = Scope::Private { user_id: "u1".to_string() };
        
        // Test clones and derives to hit code coverage for generated code
        assert_eq!(pub_scope, pub_scope.clone());
        assert_ne!(pub_scope, priv_scope);

        let json = serde_json::to_string(&pub_scope).unwrap();
        let decoded: Scope = serde_json::from_str(&json).unwrap();
        assert_eq!(pub_scope, decoded);

        let json_priv = serde_json::to_string(&priv_scope).unwrap();
        let decoded_priv: Scope = serde_json::from_str(&json_priv).unwrap();
        assert_eq!(priv_scope, decoded_priv);
    }

    #[test]
    fn test_scope_cross_boundaries() {
        let pub_scope = Scope::Public { channel_id: "c1".to_string(), user_id: "alice".to_string() };
        let priv_scope = Scope::Private { user_id: "alice".to_string() };

        // Test the structural match branches (Public/Private and Private/Public) explicitly again to be safe
        assert!(!pub_scope.can_read(&priv_scope));
        assert!(!priv_scope.can_read(&pub_scope));
    }
}
