use ampersona_core::errors::{PolicyDecision, PolicyError};
use ampersona_core::spec::authority::ScopedAction;
use ampersona_core::traits::{AuthorityEnforcer, PolicyRequest, ResolvedAuthority};

/// Default policy checker implementing deny-by-default with layered authority.
///
/// Evaluation order (inspired by zeroclaw's layered gates):
/// 1. Explicit deny check (deny always wins)
/// 2. Action allow-list check (deny-by-default for unknown)
/// 3. Scoped action enforcement (shell, git, file_access)
/// 4. Path scope check (forbidden/allowed paths)
/// 5. Autonomy level check (readonly → deny, supervised → needs approval)
pub struct DefaultPolicyChecker;

impl AuthorityEnforcer for DefaultPolicyChecker {
    fn evaluate(
        &self,
        req: &PolicyRequest,
        authority: &ResolvedAuthority,
    ) -> Result<PolicyDecision, PolicyError> {
        // 1. Explicit deny always wins
        if let Some(action) = &req.action {
            if authority.denied_actions.contains(action) {
                return Ok(PolicyDecision::Deny {
                    reason: format!("action '{action}' is explicitly denied"),
                });
            }
        }

        // 2. Check if action is in the allow list (deny-by-default)
        if let Some(action) = &req.action {
            if !authority.allowed_actions.is_empty() && !authority.allowed_actions.contains(action)
            {
                return Ok(PolicyDecision::Deny {
                    reason: format!("action '{action}' not in allow list (deny-by-default)"),
                });
            }
        }

        // 3. Scoped action enforcement
        if let Some(decision) = self.check_scoped_actions(req, authority) {
            return Ok(decision);
        }

        // 4. Check path scope
        if let Some(path) = &req.path {
            if let Some(scope) = &authority.scope {
                // Forbidden paths first (deny)
                if let Some(forbidden) = &scope.forbidden_paths {
                    for pattern in forbidden {
                        if glob_match(pattern, path) {
                            return Ok(PolicyDecision::Deny {
                                reason: format!(
                                    "path '{path}' matches forbidden pattern '{pattern}'"
                                ),
                            });
                        }
                    }
                }
                // Then allowed paths
                if let Some(allowed) = &scope.allowed_paths {
                    let path_allowed = allowed.iter().any(|p| glob_match(p, path));
                    if !path_allowed {
                        return Ok(PolicyDecision::Deny {
                            reason: format!("path '{path}' not in allowed paths"),
                        });
                    }
                }
            }
        }

        // 5. Autonomy level check
        match authority.autonomy {
            ampersona_core::types::AutonomyLevel::Readonly => {
                return Ok(PolicyDecision::Deny {
                    reason: "agent is in readonly mode".to_string(),
                });
            }
            ampersona_core::types::AutonomyLevel::Supervised => {
                if let Some(limits) = &authority.limits {
                    if limits.require_approval_for.is_some() {
                        return Ok(PolicyDecision::NeedsApproval {
                            reason: "supervised mode requires approval".to_string(),
                        });
                    }
                }
            }
            ampersona_core::types::AutonomyLevel::Full => {}
        }

        Ok(PolicyDecision::Allow {
            reason: "action permitted by authority".to_string(),
        })
    }
}

impl DefaultPolicyChecker {
    /// Check scoped action constraints (shell, git, file_access).
    fn check_scoped_actions(
        &self,
        req: &PolicyRequest,
        authority: &ResolvedAuthority,
    ) -> Option<PolicyDecision> {
        let ctx = &req.context;

        // Check shell scoped action
        if let Some(ScopedAction::Shell(shell)) = authority.scoped_actions.get("shell") {
            if let Some(cmd) = ctx.get("command").and_then(|v| v.as_str()) {
                // Check allowed commands
                if let Some(commands) = &shell.commands {
                    let base_cmd = cmd.split_whitespace().next().unwrap_or(cmd);
                    if !commands.iter().any(|c| c == base_cmd) {
                        return Some(PolicyDecision::Deny {
                            reason: format!(
                                "command '{base_cmd}' not in allowed shell commands: {}",
                                commands.join(", ")
                            ),
                        });
                    }
                }

                // Check shell injection patterns
                if shell.block_subshells.unwrap_or(false) && has_subshell(cmd) {
                    return Some(PolicyDecision::Deny {
                        reason: "subshells are blocked by shell policy".to_string(),
                    });
                }
                if shell.block_redirects.unwrap_or(false) && has_redirect(cmd) {
                    return Some(PolicyDecision::Deny {
                        reason: "redirects are blocked by shell policy".to_string(),
                    });
                }
                if shell.block_background.unwrap_or(false) && has_background(cmd) {
                    return Some(PolicyDecision::Deny {
                        reason: "background execution is blocked by shell policy".to_string(),
                    });
                }
            }
        }

        // Check git scoped action
        if let Some(ScopedAction::Git(git)) = authority.scoped_actions.get("git") {
            if let Some(operation) = ctx.get("git_operation").and_then(|v| v.as_str()) {
                if let Some(allowed_ops) = &git.allowed_operations {
                    if !allowed_ops.iter().any(|op| op == operation) {
                        return Some(PolicyDecision::Deny {
                            reason: format!(
                                "git operation '{operation}' not allowed (allowed: {})",
                                allowed_ops.join(", ")
                            ),
                        });
                    }
                }

                // Check branch restrictions for push
                if operation == "push" {
                    if let Some(branch) = ctx.get("branch").and_then(|v| v.as_str()) {
                        if let Some(deny_branches) = &git.deny_push_branches {
                            for pattern in deny_branches {
                                if glob_match(pattern, branch) {
                                    return Some(PolicyDecision::Deny {
                                        reason: format!(
                                            "push to branch '{branch}' denied by pattern '{pattern}'"
                                        ),
                                    });
                                }
                            }
                        }
                        if let Some(push_branches) = &git.push_branches {
                            let allowed = push_branches.iter().any(|p| glob_match(p, branch));
                            if !allowed {
                                return Some(PolicyDecision::Deny {
                                    reason: format!(
                                        "push to branch '{branch}' not in allowed patterns"
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Check file_access scoped action
        if let Some(ScopedAction::FileAccess(fa)) = authority.scoped_actions.get("file_access") {
            if let Some(path) = &req.path {
                let is_write = ctx
                    .get("operation")
                    .and_then(|v| v.as_str())
                    .is_some_and(|op| op == "write");

                if is_write {
                    // Check deny_write first
                    if let Some(deny_write) = &fa.deny_write {
                        for pattern in deny_write {
                            if glob_match(pattern, path) {
                                return Some(PolicyDecision::Deny {
                                    reason: format!(
                                        "write to '{path}' denied by pattern '{pattern}'"
                                    ),
                                });
                            }
                        }
                    }
                    // Check write allowed
                    if let Some(write) = &fa.write {
                        let allowed = write.iter().any(|p| glob_match(p, path));
                        if !allowed {
                            return Some(PolicyDecision::Deny {
                                reason: format!("write to '{path}' not in allowed write paths"),
                            });
                        }
                    }
                } else {
                    // Read check
                    if let Some(read) = &fa.read {
                        let allowed = read.iter().any(|p| glob_match(p, path));
                        if !allowed {
                            return Some(PolicyDecision::Deny {
                                reason: format!("read from '{path}' not in allowed read paths"),
                            });
                        }
                    }
                }
            }
        }

        None
    }
}

/// Check for subshell patterns in a command string.
fn has_subshell(cmd: &str) -> bool {
    cmd.contains("$(") || cmd.contains('`') || cmd.contains("( ")
}

/// Check for redirect patterns in a command string.
fn has_redirect(cmd: &str) -> bool {
    cmd.contains('>') || cmd.contains("< ")
}

/// Check for background execution patterns in a command string.
fn has_background(cmd: &str) -> bool {
    cmd.trim_end().ends_with('&') || cmd.contains("& ")
}

/// Simple glob matching (supports *, **, and file extensions).
fn glob_match(pattern: &str, path: &str) -> bool {
    if pattern == "**/*" || pattern == path {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path.starts_with(prefix);
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        if let Some(rest) = path.strip_prefix(prefix) {
            return rest.starts_with('/') && !rest[1..].contains('/');
        }
    }
    if let Some(ext_pattern) = pattern.strip_prefix("*.") {
        return path.ends_with(&format!(".{ext_pattern}"));
    }
    pattern == path
}

#[cfg(test)]
mod tests {
    use super::*;
    use ampersona_core::spec::authority::{ScopedFileAccess, ScopedGit, ScopedShell};
    use ampersona_core::types::AutonomyLevel;
    use std::collections::HashMap;

    fn make_authority(
        autonomy: AutonomyLevel,
        allowed: Vec<&str>,
        denied: Vec<&str>,
    ) -> ResolvedAuthority {
        ResolvedAuthority {
            autonomy,
            allowed_actions: allowed.into_iter().filter_map(|s| s.parse().ok()).collect(),
            denied_actions: denied.into_iter().filter_map(|s| s.parse().ok()).collect(),
            scope: None,
            limits: None,
            scoped_actions: HashMap::new(),
            deny_metadata: HashMap::new(),
        }
    }

    #[test]
    fn deny_wins() {
        let checker = DefaultPolicyChecker;
        let auth = make_authority(
            AutonomyLevel::Full,
            vec!["read_file", "write_file"],
            vec!["write_file"],
        );
        let req = PolicyRequest {
            action: Some("write_file".parse().unwrap()),
            path: None,
            context: HashMap::new(),
        };
        let result = checker.evaluate(&req, &auth).unwrap();
        assert!(matches!(result, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn allow_known_action() {
        let checker = DefaultPolicyChecker;
        let auth = make_authority(AutonomyLevel::Full, vec!["read_file"], vec![]);
        let req = PolicyRequest {
            action: Some("read_file".parse().unwrap()),
            path: None,
            context: HashMap::new(),
        };
        let result = checker.evaluate(&req, &auth).unwrap();
        assert!(matches!(result, PolicyDecision::Allow { .. }));
    }

    #[test]
    fn deny_by_default_unknown() {
        let checker = DefaultPolicyChecker;
        let auth = make_authority(AutonomyLevel::Full, vec!["read_file"], vec![]);
        let req = PolicyRequest {
            action: Some("deploy".parse().unwrap()),
            path: None,
            context: HashMap::new(),
        };
        let result = checker.evaluate(&req, &auth).unwrap();
        assert!(matches!(result, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn readonly_denies_all() {
        let checker = DefaultPolicyChecker;
        let auth = make_authority(AutonomyLevel::Readonly, vec![], vec![]);
        let req = PolicyRequest {
            action: None,
            path: None,
            context: HashMap::new(),
        };
        let result = checker.evaluate(&req, &auth).unwrap();
        assert!(matches!(result, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn shell_command_not_allowed() {
        let checker = DefaultPolicyChecker;
        let mut scoped = HashMap::new();
        scoped.insert(
            "shell".to_string(),
            ScopedAction::Shell(ScopedShell {
                commands: Some(vec!["git".into(), "cargo".into()]),
                block_high_risk: None,
                block_subshells: Some(true),
                block_redirects: Some(true),
                block_background: Some(true),
                validate_symlinks: None,
            }),
        );
        let auth = ResolvedAuthority {
            autonomy: AutonomyLevel::Full,
            allowed_actions: vec!["run_command".parse().unwrap()],
            denied_actions: vec![],
            scope: None,
            limits: None,
            scoped_actions: scoped,
            deny_metadata: HashMap::new(),
        };
        let mut ctx = HashMap::new();
        ctx.insert(
            "command".to_string(),
            serde_json::Value::String("rm -rf /".into()),
        );
        let req = PolicyRequest {
            action: Some("run_command".parse().unwrap()),
            path: None,
            context: ctx,
        };
        let result = checker.evaluate(&req, &auth).unwrap();
        assert!(matches!(result, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn shell_subshell_blocked() {
        let checker = DefaultPolicyChecker;
        let mut scoped = HashMap::new();
        scoped.insert(
            "shell".to_string(),
            ScopedAction::Shell(ScopedShell {
                commands: Some(vec!["echo".into()]),
                block_high_risk: None,
                block_subshells: Some(true),
                block_redirects: None,
                block_background: None,
                validate_symlinks: None,
            }),
        );
        let auth = ResolvedAuthority {
            autonomy: AutonomyLevel::Full,
            allowed_actions: vec!["run_command".parse().unwrap()],
            denied_actions: vec![],
            scope: None,
            limits: None,
            scoped_actions: scoped,
            deny_metadata: HashMap::new(),
        };
        let mut ctx = HashMap::new();
        ctx.insert(
            "command".to_string(),
            serde_json::Value::String("echo $(whoami)".into()),
        );
        let req = PolicyRequest {
            action: Some("run_command".parse().unwrap()),
            path: None,
            context: ctx,
        };
        let result = checker.evaluate(&req, &auth).unwrap();
        assert!(matches!(result, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn git_push_to_denied_branch() {
        let checker = DefaultPolicyChecker;
        let mut scoped = HashMap::new();
        scoped.insert(
            "git".to_string(),
            ScopedAction::Git(ScopedGit {
                allowed_operations: Some(vec!["push".into(), "commit".into()]),
                push_branches: Some(vec!["feature/*".into()]),
                deny_push_branches: Some(vec!["main".into()]),
            }),
        );
        let auth = ResolvedAuthority {
            autonomy: AutonomyLevel::Full,
            allowed_actions: vec!["git_push".parse().unwrap()],
            denied_actions: vec![],
            scope: None,
            limits: None,
            scoped_actions: scoped,
            deny_metadata: HashMap::new(),
        };
        let mut ctx = HashMap::new();
        ctx.insert(
            "git_operation".to_string(),
            serde_json::Value::String("push".into()),
        );
        ctx.insert(
            "branch".to_string(),
            serde_json::Value::String("main".into()),
        );
        let req = PolicyRequest {
            action: Some("git_push".parse().unwrap()),
            path: None,
            context: ctx,
        };
        let result = checker.evaluate(&req, &auth).unwrap();
        assert!(matches!(result, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn file_access_deny_write_lock() {
        let checker = DefaultPolicyChecker;
        let mut scoped = HashMap::new();
        scoped.insert(
            "file_access".to_string(),
            ScopedAction::FileAccess(ScopedFileAccess {
                read: Some(vec!["**/*".into()]),
                write: Some(vec!["src/**".into()]),
                deny_write: Some(vec!["*.lock".into()]),
            }),
        );
        let auth = ResolvedAuthority {
            autonomy: AutonomyLevel::Full,
            allowed_actions: vec!["write_file".parse().unwrap()],
            denied_actions: vec![],
            scope: None,
            limits: None,
            scoped_actions: scoped,
            deny_metadata: HashMap::new(),
        };
        let mut ctx = HashMap::new();
        ctx.insert(
            "operation".to_string(),
            serde_json::Value::String("write".into()),
        );
        let req = PolicyRequest {
            action: Some("write_file".parse().unwrap()),
            path: Some("Cargo.lock".into()),
            context: ctx,
        };
        let result = checker.evaluate(&req, &auth).unwrap();
        assert!(matches!(result, PolicyDecision::Deny { .. }));
    }
}
