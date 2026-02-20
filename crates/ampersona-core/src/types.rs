#![allow(clippy::doc_markdown)]

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A floating-point value constrained to [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct UnitFloat(f64);

impl UnitFloat {
    pub fn new(v: f64) -> Option<Self> {
        if (0.0..=1.0).contains(&v) {
            Some(Self(v))
        } else {
            None
        }
    }

    pub fn value(self) -> f64 {
        self.0
    }
}

impl fmt::Display for UnitFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

impl Serialize for UnitFloat {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for UnitFloat {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(deserializer)?;
        UnitFloat::new(v).ok_or_else(|| serde::de::Error::custom(format!("{v} not in [0.0, 1.0]")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutonomyLevel {
    Readonly,
    Supervised,
    Full,
}

impl AutonomyLevel {
    /// Returns the minimum (most restrictive) of two levels.
    pub fn min(self, other: Self) -> Self {
        use AutonomyLevel::*;
        match (self, other) {
            (Readonly, _) | (_, Readonly) => Readonly,
            (Supervised, _) | (_, Supervised) => Supervised,
            (Full, Full) => Full,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CriterionOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateApproval {
    Auto,
    Human,
    Quorum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateDirection {
    Promote,
    Demote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateEnforcement {
    #[default]
    Enforce,
    Observe,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    #[serde(rename = "low_risk")]
    LowRisk,
    #[serde(rename = "medium_risk")]
    MediumRisk,
    #[serde(rename = "high_risk")]
    HighRisk,
}

/// Scoped action discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScopedType {
    Shell,
    Git,
    #[serde(rename = "file_access")]
    FileAccess,
    Custom,
}

/// Audit event taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    PolicyDecision,
    GateTransition,
    ElevationChange,
    Override,
    SignatureVerify,
    StateChange,
    AuthorityOverlayChange,
}

/// MBTI personality types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MbtiType {
    ISTJ,
    ISFJ,
    INFJ,
    INTJ,
    ISTP,
    ISFP,
    INFP,
    INTP,
    ESTP,
    ESFP,
    ENFP,
    ENTP,
    ESTJ,
    ESFJ,
    ENFJ,
    ENTJ,
}

/// D&D alignment system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Alignment {
    LawfulGood,
    NeutralGood,
    ChaoticGood,
    LawfulNeutral,
    TrueNeutral,
    ChaoticNeutral,
    LawfulEvil,
    NeutralEvil,
    ChaoticEvil,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_float_bounds() {
        assert!(UnitFloat::new(0.0).is_some());
        assert!(UnitFloat::new(1.0).is_some());
        assert!(UnitFloat::new(0.5).is_some());
        assert!(UnitFloat::new(-0.1).is_none());
        assert!(UnitFloat::new(1.1).is_none());
    }

    #[test]
    fn autonomy_min() {
        assert_eq!(
            AutonomyLevel::Full.min(AutonomyLevel::Supervised),
            AutonomyLevel::Supervised
        );
        assert_eq!(
            AutonomyLevel::Supervised.min(AutonomyLevel::Readonly),
            AutonomyLevel::Readonly
        );
        assert_eq!(
            AutonomyLevel::Full.min(AutonomyLevel::Full),
            AutonomyLevel::Full
        );
    }

    #[test]
    fn serde_roundtrip_autonomy() {
        let json = serde_json::to_string(&AutonomyLevel::Supervised).unwrap();
        assert_eq!(json, "\"supervised\"");
        let parsed: AutonomyLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, AutonomyLevel::Supervised);
    }
}
