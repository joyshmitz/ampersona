use serde_json::Value;

/// Merge two persona JSON values (base + overlay).
///
/// Overlay fields take precedence. For authority, merge rules apply:
/// - deny = union
/// - allow = intersection minus deny
/// - limits = minimum
/// - autonomy = minimum
pub fn merge_personas(base: &Value, overlay: &Value) -> Value {
    let mut result = base.clone();
    if let (Some(base_obj), Some(overlay_obj)) = (result.as_object_mut(), overlay.as_object()) {
        for (key, value) in overlay_obj {
            if key == "authority" {
                if let Some(base_auth) = base_obj.get("authority") {
                    base_obj.insert(key.clone(), merge_authority(base_auth, value));
                } else {
                    base_obj.insert(key.clone(), value.clone());
                }
            } else {
                base_obj.insert(key.clone(), value.clone());
            }
        }
    }
    result
}

fn merge_authority(base: &Value, overlay: &Value) -> Value {
    let mut result = base.clone();
    if let (Some(base_obj), Some(overlay_obj)) = (result.as_object_mut(), overlay.as_object()) {
        for (key, value) in overlay_obj {
            match key.as_str() {
                "autonomy" => {
                    // Minimum autonomy
                    let base_level = base_obj
                        .get("autonomy")
                        .and_then(Value::as_str)
                        .unwrap_or("full");
                    let overlay_level = value.as_str().unwrap_or("full");
                    let min = min_autonomy_str(base_level, overlay_level);
                    base_obj.insert(key.clone(), Value::String(min.to_string()));
                }
                "actions" => {
                    if let Some(base_actions) = base_obj.get("actions") {
                        base_obj.insert(key.clone(), merge_actions(base_actions, value));
                    } else {
                        base_obj.insert(key.clone(), value.clone());
                    }
                }
                "limits" => {
                    if let Some(base_limits) = base_obj.get("limits") {
                        base_obj.insert(key.clone(), merge_limits(base_limits, value));
                    } else {
                        base_obj.insert(key.clone(), value.clone());
                    }
                }
                _ => {
                    base_obj.insert(key.clone(), value.clone());
                }
            }
        }
    }
    result
}

fn merge_actions(base: &Value, overlay: &Value) -> Value {
    let mut result = base.clone();
    if let (Some(result_obj), Some(overlay_obj)) = (result.as_object_mut(), overlay.as_object()) {
        // Deny = union
        if let Some(overlay_deny) = overlay_obj.get("deny").and_then(Value::as_array) {
            let mut deny = result_obj
                .get("deny")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            deny.extend(overlay_deny.iter().cloned());
            result_obj.insert("deny".to_string(), Value::Array(deny));
        }

        // Allow from overlay replaces (intersection is done at resolve time)
        if let Some(overlay_allow) = overlay_obj.get("allow") {
            result_obj.insert("allow".to_string(), overlay_allow.clone());
        }

        // Scoped = merge
        if let Some(overlay_scoped) = overlay_obj.get("scoped") {
            if let Some(base_scoped) = result_obj.get_mut("scoped") {
                if let (Some(bs), Some(os)) =
                    (base_scoped.as_object_mut(), overlay_scoped.as_object())
                {
                    for (k, v) in os {
                        bs.insert(k.clone(), v.clone());
                    }
                }
            } else {
                result_obj.insert("scoped".to_string(), overlay_scoped.clone());
            }
        }
    }
    result
}

fn merge_limits(base: &Value, overlay: &Value) -> Value {
    let mut result = base.clone();
    if let (Some(result_obj), Some(overlay_obj)) = (result.as_object_mut(), overlay.as_object()) {
        for (key, value) in overlay_obj {
            if let (Some(base_val), Some(overlay_val)) =
                (result_obj.get(key).and_then(Value::as_u64), value.as_u64())
            {
                // Minimum for numeric limits
                result_obj.insert(
                    key.clone(),
                    Value::Number(serde_json::Number::from(base_val.min(overlay_val))),
                );
            } else {
                result_obj.insert(key.clone(), value.clone());
            }
        }
    }
    result
}

fn min_autonomy_str(a: &str, b: &str) -> &'static str {
    let rank = |s: &str| match s {
        "readonly" => 0,
        "supervised" => 1,
        "full" => 2,
        _ => 2,
    };
    match rank(a).min(rank(b)) {
        0 => "readonly",
        1 => "supervised",
        _ => "full",
    }
}
