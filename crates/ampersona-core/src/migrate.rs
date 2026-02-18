use anyhow::{bail, Result};
use serde_json::Value;

use crate::schema::detect_version;

/// Migrate a persona from v0.2 to v1.0.
///
/// Returns the migrated value. If already v1.0, returns as-is.
pub fn migrate_to_v1(data: &Value) -> Result<Value> {
    let version = detect_version(data);
    if version == "1.0" {
        return Ok(data.clone());
    }

    if version != "0.2" {
        bail!("unsupported version for migration: {version}");
    }

    let mut migrated = data.clone();
    let obj = migrated
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("persona must be a JSON object"))?;

    // Add version field
    obj.insert(
        "version".to_string(),
        serde_json::Value::String("1.0".to_string()),
    );

    // Add $schema URI
    obj.insert(
        "$schema".to_string(),
        serde_json::Value::String(
            "https://ampersona.dev/schema/v1.0/ampersona.schema.json".to_string(),
        ),
    );

    Ok(migrated)
}

/// Migrate a file in-place.
pub fn migrate_file(path: &str) -> Result<()> {
    let content =
        std::fs::read_to_string(path).map_err(|e| anyhow::anyhow!("cannot read {path}: {e}"))?;
    let data: Value =
        serde_json::from_str(&content).map_err(|e| anyhow::anyhow!("{path}: invalid JSON: {e}"))?;

    let version = detect_version(&data);
    if version == "1.0" {
        eprintln!("  skip {path} (already v1.0)");
        return Ok(());
    }

    let migrated = migrate_to_v1(&data)?;
    let json = serde_json::to_string_pretty(&migrated)?;
    std::fs::write(path, json)?;
    eprintln!("  migrated {path} (v0.2 → v1.0)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn migrate_adds_version() {
        let v02 = json!({
            "name": "Test",
            "role": "Tester",
            "psychology": {
                "neural_matrix": {
                    "creativity": 0.5, "empathy": 0.5, "logic": 0.5,
                    "adaptability": 0.5, "charisma": 0.5, "reliability": 0.5
                },
                "traits": {
                    "ocean": {
                        "openness": 0.5, "conscientiousness": 0.5,
                        "extraversion": 0.5, "agreeableness": 0.5, "neuroticism": 0.5
                    },
                    "mbti": "INTJ"
                }
            },
            "voice": {
                "style": {
                    "descriptors": ["clear"],
                    "formality": 0.5,
                    "verbosity": 0.5
                }
            }
        });

        let migrated = migrate_to_v1(&v02).unwrap();
        assert_eq!(migrated.get("version").unwrap().as_str().unwrap(), "1.0");
        assert!(migrated.get("$schema").is_some());
    }

    #[test]
    fn migrate_rejects_non_object() {
        let array = json!([1, 2, 3]);
        let result = migrate_to_v1(&array);
        assert!(result.is_err());
    }

    #[test]
    fn migrate_handles_empty_object() {
        let empty = json!({});
        let result = migrate_to_v1(&empty).unwrap();
        assert_eq!(result["version"], "1.0");
    }

    #[test]
    fn migrate_idempotent() {
        let v10 = json!({
            "version": "1.0",
            "name": "Test",
            "role": "Tester",
            "psychology": { "neural_matrix": {}, "traits": { "ocean": {}, "mbti": "INTJ" } },
            "voice": { "style": { "descriptors": ["x"], "formality": 0.5, "verbosity": 0.5 } }
        });

        let result = migrate_to_v1(&v10).unwrap();
        assert_eq!(result, v10);
    }
}
