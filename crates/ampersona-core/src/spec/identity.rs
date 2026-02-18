use serde::{Deserialize, Serialize};

use crate::types::{Alignment, MbtiType, UnitFloat};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Psychology {
    pub neural_matrix: NeuralMatrix,
    pub traits: Traits,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moral_compass: Option<MoralCompass>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emotional_profile: Option<EmotionalProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralMatrix {
    pub creativity: UnitFloat,
    pub empathy: UnitFloat,
    pub logic: UnitFloat,
    pub adaptability: UnitFloat,
    pub charisma: UnitFloat,
    pub reliability: UnitFloat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Traits {
    pub ocean: Ocean,
    pub mbti: MbtiType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperament: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ocean {
    pub openness: UnitFloat,
    pub conscientiousness: UnitFloat,
    pub extraversion: UnitFloat,
    pub agreeableness: UnitFloat,
    pub neuroticism: UnitFloat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoralCompass {
    pub alignment: Alignment,
    pub core_values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalProfile {
    pub base_mood: String,
    pub volatility: UnitFloat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voice {
    pub style: VoiceStyle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax: Option<VoiceSyntax>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idiolect: Option<Idiolect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tts: Option<TtsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceStyle {
    pub descriptors: Vec<String>,
    pub formality: UnitFloat,
    pub verbosity: UnitFloat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceSyntax {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contractions: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Idiolect {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catchphrases: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forbidden_words: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    pub provider: String,
    pub voice_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<UnitFloat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity_boost: Option<UnitFloat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<UnitFloat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub skills: Vec<Skill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Directives {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub core_drive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goals: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<Vec<String>>,
}
