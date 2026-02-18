use ampersona_core::actions::{ActionId, BuiltinAction};

/// Validate that an action ID is recognized.
pub fn validate_action(id: &ActionId) -> bool {
    match id {
        ActionId::Builtin(_) => true,
        ActionId::Custom { vendor, .. } => vendor != "_unknown",
    }
}

/// Check if an action string is a known builtin.
pub fn is_builtin(name: &str) -> bool {
    BuiltinAction::from_str_opt(name).is_some()
}
