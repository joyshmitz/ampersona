#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io::{self, Read};

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "amp",
    version,
    about = "Agent identity, authority, and trust gates. Unix-friendly."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a Markdown system prompt from a persona JSON.
    Prompt {
        /// Path to persona .json (or "-" / omit for stdin).
        #[arg(default_value = "-")]
        file: String,

        /// Output TOON instead of Markdown.
        #[arg(long)]
        toon: bool,

        /// Include only these sections (comma-separated).
        #[arg(long, value_delimiter = ',')]
        sections: Vec<String>,
    },

    /// Validate persona JSON files against the ampersona schema.
    Validate {
        /// One or more .json file paths.
        #[arg(required = true)]
        files: Vec<String>,
    },

    /// Create a new persona from a built-in template.
    New {
        /// Template name: architect, worker, scout.
        template: String,

        /// Set the persona name (AdjectiveNoun).
        #[arg(long)]
        name: Option<String>,

        /// Write to file instead of stdout.
        #[arg(short, long)]
        output: Option<String>,
    },

    /// List available built-in templates.
    Templates,

    /// Summarize persona files in a directory as a table.
    List {
        /// Directory containing .json persona files.
        #[arg(default_value = ".")]
        dir: String,
    },

    /// Generate a register_agent MCP call from a persona JSON.
    Register {
        /// Path to persona .json (or "-" / omit for stdin).
        #[arg(default_value = "-")]
        file: String,

        /// mcp_agent_mail project key (absolute path).
        #[arg(long)]
        project: String,

        /// Agent program name.
        #[arg(long, default_value = "amp")]
        program: String,

        /// Agent model name.
        #[arg(long, default_value = "persona-driven")]
        model: String,

        /// Include full system prompt in task_description.
        #[arg(long)]
        prompt: bool,

        /// Use TOON format for task_description (implies --prompt).
        #[arg(long)]
        toon: bool,

        /// Wrap output in JSON-RPC 2.0 envelope.
        #[arg(long)]
        rpc: bool,
    },

    /// Bootstrap a persona file or workspace.
    Init {
        /// Initialize workspace defaults (.ampersona/defaults.json).
        #[arg(long)]
        workspace: bool,
    },

    /// Unified validation: schema + consistency + action vocab + lint.
    Check {
        /// Path to persona .json file.
        file: String,

        /// Output structured JSON report.
        #[arg(long)]
        json: bool,

        /// Fail on warnings (not just errors).
        #[arg(long)]
        strict: bool,
    },

    /// Migrate persona files from v0.2 to v1.0.
    Migrate {
        /// One or more .json file paths.
        #[arg(required = true)]
        files: Vec<String>,
    },

    /// Show phase, autonomy, elevations, and drift.
    Status {
        /// Path to persona .json file.
        file: String,

        /// Output JSON.
        #[arg(long)]
        json: bool,

        /// Show drift trend.
        #[arg(long)]
        drift: bool,
    },

    /// Check if an action is allowed by authority.
    Authority {
        /// Path to persona .json file.
        file: String,

        /// Action to check.
        #[arg(long)]
        check: String,

        /// Output structured JSON.
        #[arg(long)]
        json: bool,

        /// Resource path for scope check.
        #[arg(long)]
        path: Option<String>,

        /// Context key=value pairs for scoped actions.
        #[arg(long, value_parser = parse_context_kv)]
        context: Vec<(String, String)>,

        /// Context as JSON object (merged with --context).
        #[arg(long)]
        context_json: Option<String>,
    },

    /// Activate a temporary elevation.
    Elevate {
        /// Path to persona .json file.
        file: String,

        /// Elevation ID.
        #[arg(long)]
        elevation: String,

        /// Reason for elevation.
        #[arg(long)]
        reason: String,
    },

    /// Evaluate or override a gate.
    Gate {
        /// Path to persona .json file.
        file: String,

        /// Gate ID to evaluate.
        #[arg(long)]
        evaluate: Option<String>,

        /// Metrics file for evaluation.
        #[arg(long)]
        metrics: Option<String>,

        /// Gate ID to override.
        #[arg(long = "override")]
        override_gate: Option<String>,

        /// Reason for override.
        #[arg(long)]
        reason: Option<String>,

        /// Approver for override.
        #[arg(long)]
        approver: Option<String>,

        /// Approve a pending human gate transition.
        #[arg(long)]
        approve: Option<String>,

        /// Output structured JSON.
        #[arg(long)]
        json: bool,
    },

    /// Sign a persona file.
    Sign {
        /// Path to persona .json file.
        file: String,

        /// Path to ed25519 private key.
        #[arg(long)]
        key: String,

        /// Key identifier for rotation.
        #[arg(long, default_value = "default")]
        key_id: String,
    },

    /// Verify a persona signature.
    Verify {
        /// Path to persona .json file.
        file: String,

        /// Path to ed25519 public key.
        #[arg(long)]
        pubkey: String,
    },

    /// Verify audit log hash-chain.
    Audit {
        /// Path to persona .json file.
        file: String,

        /// Verify the hash chain.
        #[arg(long)]
        verify: bool,

        /// Start verification from entry N (0-based).
        #[arg(long)]
        from: Option<u64>,

        /// Create an integrity checkpoint.
        #[arg(long)]
        checkpoint_create: bool,

        /// Verify an existing checkpoint.
        #[arg(long)]
        checkpoint_verify: bool,

        /// Path to checkpoint file (for --checkpoint-create / --checkpoint-verify).
        #[arg(long)]
        checkpoint: Option<String>,

        /// Sign the checkpoint with this ed25519 private key.
        #[arg(long)]
        sign_key: Option<String>,

        /// Key identifier for signed checkpoints.
        #[arg(long, default_value = "default")]
        sign_key_id: String,

        /// Public key to verify signed checkpoint.
        #[arg(long)]
        verify_key: Option<String>,

        /// Output structured JSON.
        #[arg(long)]
        json: bool,
    },

    /// Merge two personas (base + overlay).
    Compose {
        /// Base persona file.
        base: String,

        /// Overlay persona file.
        overlay: String,
    },

    /// Compare two personas.
    Diff {
        /// First persona file.
        a: String,
        /// Second persona file.
        b: String,
    },

    /// Import from external format.
    Import {
        /// Path to external file.
        file: String,

        /// Source format.
        #[arg(long)]
        from: String,
    },

    /// Export to external format.
    Export {
        /// Path to persona .json file.
        file: String,

        /// Target format.
        #[arg(long)]
        to: String,
    },

    /// Fleet-level operations.
    Fleet {
        /// Directory containing persona files.
        dir: String,

        /// Show status summary.
        #[arg(long)]
        status: bool,

        /// Run check on all files.
        #[arg(long)]
        check: bool,

        /// Output JSON report.
        #[arg(long)]
        json: bool,

        /// Apply authority overlay to all.
        #[arg(long)]
        apply_overlay: Option<String>,
    },
}

/// Parse "key=value" pairs for --context.
fn parse_context_kv(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid context: no `=` found in `{s}`"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

// ── CmdExit: structured exit for commands with semantic exit codes ──

enum CmdExit {
    Ok,
    Code(i32),
    Err(anyhow::Error),
    JsonErr {
        code: &'static str,
        message: String,
        json: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.cmd {
        Cmd::Authority {
            file,
            check,
            json,
            path,
            context,
            context_json,
        } => cmd_authority(&file, &check, json, path, context, context_json),

        Cmd::Gate {
            file,
            evaluate,
            metrics,
            override_gate,
            reason,
            approver,
            approve,
            json,
        } => cmd_gate(GateOpts {
            file,
            evaluate,
            metrics_file: metrics,
            override_gate,
            reason,
            approver,
            approve,
            json_out: json,
        }),

        Cmd::Audit {
            file,
            verify,
            from,
            checkpoint_create,
            checkpoint_verify,
            checkpoint,
            sign_key,
            sign_key_id,
            verify_key,
            json,
        } => cmd_audit(AuditOpts {
            file,
            verify,
            from,
            checkpoint_create,
            checkpoint_verify,
            checkpoint_path: checkpoint,
            sign_key,
            sign_key_id,
            verify_key,
            json_out: json,
        }),

        other => match run_other(other) {
            Ok(()) => CmdExit::Ok,
            Err(e) => CmdExit::Err(e),
        },
    };

    match result {
        CmdExit::Ok => {}
        CmdExit::Code(n) => std::process::exit(n),
        CmdExit::Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
        CmdExit::JsonErr {
            code,
            message,
            json,
        } => {
            if json {
                let err = serde_json::json!({
                    "error": true,
                    "code": code,
                    "message": message,
                });
                println!("{}", serde_json::to_string_pretty(&err).unwrap());
            } else {
                eprintln!("error: {message}");
            }
            std::process::exit(3);
        }
    }
}

/// Dispatch commands that return Result<()> (no special exit codes).
fn run_other(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Prompt {
            file,
            toon,
            sections,
        } => cmd_prompt(&file, toon, &sections),
        Cmd::Validate { files } => cmd_validate(&files),
        Cmd::New {
            template,
            name,
            output,
        } => cmd_new(&template, name.as_deref(), output.as_deref()),
        Cmd::Templates => cmd_templates(),
        Cmd::List { dir } => cmd_list(&dir),
        Cmd::Register {
            file,
            project,
            program,
            model,
            prompt,
            toon,
            rpc,
        } => cmd_register(&file, &project, &program, &model, prompt, toon, rpc),
        Cmd::Init { workspace } => cmd_init(workspace),
        Cmd::Check { file, json, strict } => cmd_check(&file, json, strict),
        Cmd::Migrate { files } => cmd_migrate(&files),
        Cmd::Status { file, json, drift } => cmd_status(&file, json, drift),
        Cmd::Elevate {
            file,
            elevation,
            reason,
        } => cmd_elevate(&file, &elevation, &reason),
        Cmd::Sign { file, key, key_id } => cmd_sign(&file, &key, &key_id),
        Cmd::Verify { file, pubkey } => cmd_verify(&file, &pubkey),
        Cmd::Compose { base, overlay } => cmd_compose(&base, &overlay),
        Cmd::Diff { a, b } => cmd_diff(&a, &b),
        Cmd::Import { file, from } => cmd_import(&file, &from),
        Cmd::Export { file, to } => cmd_export(&file, &to),
        Cmd::Fleet {
            dir,
            status,
            check,
            json,
            apply_overlay,
        } => cmd_fleet(&dir, status, check, json, apply_overlay),
        // Authority, Gate, Audit are handled in main() directly
        _ => unreachable!(),
    }
}

// ── Existing commands (migrated from v0.2) ──────────────────────

fn read_persona(file: &str) -> Result<serde_json::Value> {
    if file == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        Ok(serde_json::from_str(&buf)?)
    } else {
        ampersona_core::prompt::load_persona(file)
    }
}

fn cmd_prompt(file: &str, toon_out: bool, sections: &[String]) -> Result<()> {
    let data = read_persona(file)?;
    if toon_out {
        println!("{}", ampersona_core::prompt::to_toon(&data)?);
    } else {
        print!(
            "{}",
            ampersona_core::prompt::to_system_prompt(&data, sections)
        );
    }
    Ok(())
}

fn cmd_validate(files: &[String]) -> Result<()> {
    let (passed, failed) = ampersona_core::schema::validate_files(files)?;
    eprintln!("\n{passed} passed, {failed} failed");
    if failed > 0 {
        bail!("{failed} file(s) failed validation");
    }
    Ok(())
}

fn cmd_new(template: &str, name: Option<&str>, output: Option<&str>) -> Result<()> {
    let persona = ampersona_core::templates::generate(template, name).ok_or_else(|| {
        let available: Vec<_> = ampersona_core::templates::list_templates()
            .iter()
            .map(|(n, _)| *n)
            .collect();
        anyhow::anyhow!(
            "unknown template \"{template}\". available: {}",
            available.join(", ")
        )
    })?;

    let json = serde_json::to_string_pretty(&persona)?;

    if let Some(path) = output {
        std::fs::write(path, &json)?;
        eprintln!("wrote {path}");
    } else {
        println!("{json}");
    }
    Ok(())
}

fn cmd_templates() -> Result<()> {
    for (name, desc) in ampersona_core::templates::list_templates() {
        println!("  {name:<12} {desc}");
    }
    Ok(())
}

fn cmd_list(dir: &str) -> Result<()> {
    let rows = ampersona_core::list::scan_dir(dir)?;
    ampersona_core::list::print_table(&rows);
    Ok(())
}

fn cmd_register(
    file: &str,
    project: &str,
    program: &str,
    model: &str,
    include_prompt: bool,
    toon: bool,
    rpc: bool,
) -> Result<()> {
    let data = read_persona(file)?;
    let include_prompt = include_prompt || toon;
    let args =
        ampersona_core::register::build_args(&data, project, program, model, include_prompt, toon)?;
    let output = if rpc {
        ampersona_core::register::wrap_rpc(args)
    } else {
        args
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

// ── New v1.0 commands ───────────────────────────────────────────

fn cmd_init(workspace: bool) -> Result<()> {
    if workspace {
        std::fs::create_dir_all(".ampersona")?;
        let defaults = serde_json::json!({
            "authority": {
                "autonomy": "supervised"
            }
        });
        let json = serde_json::to_string_pretty(&defaults)?;
        std::fs::write(".ampersona/defaults.json", &json)?;
        eprintln!("created .ampersona/defaults.json");
    } else {
        let persona = ampersona_core::templates::generate("worker", Some("NewAgent")).unwrap();
        let json = serde_json::to_string_pretty(&persona)?;
        std::fs::write("persona.json", &json)?;
        eprintln!("created persona.json (edit to customize)");
    }
    Ok(())
}

fn cmd_check(file: &str, json_out: bool, strict: bool) -> Result<()> {
    let content =
        std::fs::read_to_string(file).map_err(|e| anyhow::anyhow!("cannot read {file}: {e}"))?;
    let data: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| anyhow::anyhow!("{file}: invalid JSON: {e}"))?;

    let report = ampersona_core::schema::check(&data, file, strict);

    if json_out {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        if report.pass {
            eprintln!("  ok  {file} (v{})", report.version);
        } else {
            eprintln!("  FAIL {file} (v{})", report.version);
        }
        for e in &report.errors {
            eprintln!(
                "  error {}: {} {}",
                e.code,
                e.message,
                e.path.as_deref().unwrap_or("")
            );
        }
        for w in &report.warnings {
            eprintln!(
                "  warn  {}: {} {}",
                w.code,
                w.message,
                w.path.as_deref().unwrap_or("")
            );
        }
    }

    if !report.pass {
        bail!("check failed for {file}");
    }
    Ok(())
}

fn cmd_migrate(files: &[String]) -> Result<()> {
    for file in files {
        ampersona_core::migrate::migrate_file(file)?;
    }
    Ok(())
}

fn cmd_status(file: &str, json_out: bool, drift: bool) -> Result<()> {
    let data = read_persona(file)?;
    let name = data
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let version = ampersona_core::schema::detect_version(&data);
    let autonomy = data
        .pointer("/authority/autonomy")
        .and_then(|v| v.as_str())
        .unwrap_or("n/a");

    // Try to load state file
    let state_path = file.replace(".json", ".state.json");
    let state = ampersona_engine::state::phase::load_state(&state_path).ok();

    // Load drift entries if requested
    let drift_entries = if drift {
        let drift_path = file.replace(".json", ".drift.jsonl");
        ampersona_engine::state::drift::read_drift_entries(&drift_path).unwrap_or_default()
    } else {
        Vec::new()
    };

    if json_out {
        let mut status = serde_json::json!({
            "name": name,
            "version": version,
            "autonomy": autonomy,
            "phase": state.as_ref().and_then(|s| s.current_phase.as_deref()),
            "state_rev": state.as_ref().map(|s| s.state_rev),
            "active_elevations": state.as_ref().map(|s| s.active_elevations.len()).unwrap_or(0),
        });
        if drift {
            status["drift_entries"] = serde_json::json!(drift_entries.len());
            if let Some(last) = drift_entries.last() {
                status["last_drift"] = last.clone();
            }
        }
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        eprintln!("  Name:      {name}");
        eprintln!("  Version:   {version}");
        eprintln!("  Autonomy:  {autonomy}");
        if let Some(s) = &state {
            eprintln!(
                "  Phase:     {}",
                s.current_phase.as_deref().unwrap_or("(none)")
            );
            eprintln!("  State rev: {}", s.state_rev);
            eprintln!("  Elevations: {}", s.active_elevations.len());
        } else {
            eprintln!("  Phase:     (no state file)");
        }
        if drift {
            eprintln!("  Drift entries: {}", drift_entries.len());
            // Show last 5 entries as trend
            let start = drift_entries.len().saturating_sub(5);
            for entry in &drift_entries[start..] {
                if let Some(obj) = entry.as_object() {
                    let ts = obj.get("ts").and_then(|v| v.as_str()).unwrap_or("?");
                    let metrics = obj
                        .get("metrics")
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "{}".into());
                    eprintln!("    {ts}: {metrics}");
                }
            }
        }
    }
    Ok(())
}

fn cmd_authority(
    file: &str,
    action: &str,
    json_out: bool,
    path: Option<String>,
    context_kvs: Vec<(String, String)>,
    context_json: Option<String>,
) -> CmdExit {
    // Read persona file with structured error handling
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            return CmdExit::JsonErr {
                code: "E_FILE_NOT_FOUND",
                message: format!("cannot read {file}: {e}"),
                json: json_out,
            };
        }
    };
    let data: serde_json::Value = match serde_json::from_str(&content) {
        Ok(d) => d,
        Err(e) => {
            return CmdExit::JsonErr {
                code: "E_INVALID_JSON",
                message: format!("{file}: invalid JSON: {e}"),
                json: json_out,
            };
        }
    };
    let persona: ampersona_core::spec::Persona = match serde_json::from_value(data.clone()) {
        Ok(p) => p,
        Err(e) => {
            return CmdExit::JsonErr {
                code: "E_INVALID_PERSONA",
                message: format!("{file}: invalid persona: {e}"),
                json: json_out,
            };
        }
    };

    // Build context from --context and --context-json
    let mut ctx: HashMap<String, serde_json::Value> = HashMap::new();
    for (k, v) in &context_kvs {
        ctx.insert(k.clone(), serde_json::Value::String(v.clone()));
    }
    if let Some(cj) = &context_json {
        if let Ok(serde_json::Value::Object(obj)) = serde_json::from_str::<serde_json::Value>(cj) {
            for (k, v) in obj {
                ctx.insert(k, v);
            }
        }
    }

    let (decision, resolved) = if let Some(authority) = &persona.authority {
        let mut layers: Vec<&ampersona_core::spec::authority::Authority> = Vec::new();
        let workspace_defaults = ampersona_engine::policy::precedence::load_workspace_defaults();
        if let Some(ref wd) = workspace_defaults {
            layers.push(wd);
        }
        layers.push(authority);

        // Overlay is no longer a merge layer — it's applied as a post-resolution patch.
        // See ADR-010: authority_overlay uses patch-replace semantics.

        let state_path = file.replace(".json", ".state.json");
        let state = ampersona_engine::state::phase::load_state(&state_path).ok();

        let resolved = if let Some(ref s) = state {
            let elevation_defs = authority.elevations.as_deref().unwrap_or(&[]);
            ampersona_engine::policy::precedence::resolve_with_elevations(
                &layers,
                &s.active_elevations,
                elevation_defs,
            )
        } else {
            ampersona_engine::policy::precedence::resolve_authority(&layers)
        };

        // Apply authority overlay as post-resolution patch (ADR-010).
        // First check state.active_overlay; fall back to legacy sidecar file for migration.
        let overlay_from_state = state.as_ref().and_then(|s| s.active_overlay.as_ref());
        let sidecar_path = file.replace(".json", ".authority_overlay.json");
        let sidecar_overlay: Option<ampersona_core::spec::authority::AuthorityOverlay> =
            if overlay_from_state.is_none() {
                std::fs::read_to_string(&sidecar_path)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
            } else {
                None
            };
        let active_overlay = overlay_from_state.or(sidecar_overlay.as_ref());
        let resolved = if let Some(overlay) = active_overlay {
            ampersona_engine::policy::precedence::apply_overlay(&resolved, overlay)
        } else {
            resolved
        };

        let checker = ampersona_engine::policy::checker::DefaultPolicyChecker;

        use ampersona_core::traits::AuthorityEnforcer;
        let req = ampersona_core::traits::PolicyRequest {
            action: Some(action.parse().unwrap_or_else(|_| {
                ampersona_core::actions::ActionId::Custom {
                    vendor: "_unknown".into(),
                    action: action.into(),
                }
            })),
            path: path.clone(),
            context: ctx.clone(),
        };
        match checker.evaluate(&req, &resolved) {
            Ok(d) => (d, Some(resolved)),
            Err(e) => {
                return CmdExit::JsonErr {
                    code: "E_INTERNAL",
                    message: format!("policy evaluation error: {e}"),
                    json: json_out,
                };
            }
        }
    } else {
        (
            ampersona_core::errors::PolicyDecision::Deny {
                reason: "no authority section defined".to_string(),
            },
            None,
        )
    };

    // Determine exit code
    let exit_code = match &decision {
        ampersona_core::errors::PolicyDecision::Allow { .. } => 0,
        ampersona_core::errors::PolicyDecision::Deny { .. } => 1,
        ampersona_core::errors::PolicyDecision::NeedsApproval { .. } => 2,
    };

    if json_out {
        let (decision_str, reason) = match &decision {
            ampersona_core::errors::PolicyDecision::Allow { reason } => ("Allow", reason.clone()),
            ampersona_core::errors::PolicyDecision::Deny { reason } => ("Deny", reason.clone()),
            ampersona_core::errors::PolicyDecision::NeedsApproval { reason } => {
                ("NeedsApproval", reason.clone())
            }
        };

        // Look up deny metadata
        let deny_entry = resolved
            .as_ref()
            .and_then(|r| r.deny_metadata.get(action))
            .map(|m| {
                serde_json::json!({
                    "reason": m.reason,
                    "compliance_ref": m.compliance_ref,
                })
            });

        let autonomy_str = resolved
            .as_ref()
            .map(|r| format!("{:?}", r.autonomy).to_lowercase())
            .unwrap_or_else(|| "n/a".into());

        let output = serde_json::json!({
            "action": action,
            "decision": decision_str,
            "reason": reason,
            "autonomy": autonomy_str,
            "deny_entry": deny_entry,
            "path": path,
            "context": ctx,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("{decision}");
    }

    if exit_code == 0 {
        CmdExit::Ok
    } else {
        CmdExit::Code(exit_code)
    }
}

fn cmd_elevate(file: &str, elevation_id: &str, reason: &str) -> Result<()> {
    let data = read_persona(file)?;
    let persona: ampersona_core::spec::Persona = serde_json::from_value(data)?;

    let elev = persona
        .authority
        .as_ref()
        .and_then(|a| a.elevations.as_ref())
        .and_then(|elevs| elevs.iter().find(|e| e.id == elevation_id))
        .ok_or_else(|| anyhow::anyhow!("elevation '{elevation_id}' not found"))?;

    let state_path = file.replace(".json", ".state.json");
    let writer = ampersona_engine::state::writer::StateWriter::acquire(&state_path);
    let mut state = ampersona_engine::state::phase::load_state(&state_path)
        .unwrap_or_else(|_| ampersona_core::state::PhaseState::new(persona.name.clone()));

    // Enforce TTL on existing elevations
    let expired = ampersona_engine::state::elevation::enforce_ttl(&mut state);
    for eid in &expired {
        eprintln!("  elevation '{eid}' expired");
    }

    ampersona_engine::state::elevation::activate(
        &mut state,
        elevation_id,
        elev.ttl_seconds as i64,
        reason,
        "cli",
    );
    state.state_rev += 1;
    state.updated_at = chrono::Utc::now();

    // Audit the elevation change
    let audit_entry = serde_json::json!({
        "event_type": "ElevationChange",
        "elevation_id": elevation_id,
        "action": "activate",
        "reason": reason,
        "ttl_seconds": elev.ttl_seconds,
        "granted_by": "cli",
        "state_rev": state.state_rev,
    });

    if let Ok(ref w) = writer {
        w.maybe_audit(persona.audit.as_ref(), "ElevationChange", &audit_entry)?;
        w.write_state(&state)?;
    } else {
        // Fallback: write without lock (backward compat)
        let json = serde_json::to_string_pretty(&state)?;
        ampersona_engine::state::atomic::atomic_write(&state_path, json.as_bytes())?;
    }

    eprintln!(
        "  elevation '{elevation_id}' activated (TTL: {}s)",
        elev.ttl_seconds
    );
    Ok(())
}

struct GateOpts {
    file: String,
    evaluate: Option<String>,
    metrics_file: Option<String>,
    override_gate: Option<String>,
    reason: Option<String>,
    approver: Option<String>,
    approve: Option<String>,
    json_out: bool,
}

fn cmd_gate(opts: GateOpts) -> CmdExit {
    let json_out = opts.json_out;
    match cmd_gate_inner(opts) {
        Ok(exit) => exit,
        Err(e) => CmdExit::JsonErr {
            code: "E_INTERNAL",
            message: format!("{e:#}"),
            json: json_out,
        },
    }
}

fn cmd_gate_inner(opts: GateOpts) -> Result<CmdExit> {
    let GateOpts {
        ref file,
        evaluate,
        metrics_file,
        override_gate,
        reason,
        approver,
        approve,
        json_out,
    } = opts;
    let data = read_persona(file)?;
    let persona: ampersona_core::spec::Persona = serde_json::from_value(data)?;

    // Handle --approve: apply a pending transition
    if let Some(gate_id) = approve {
        let state_path = file.replace(".json", ".state.json");
        let writer = ampersona_engine::state::writer::StateWriter::acquire(&state_path);
        let mut state = ampersona_engine::state::phase::load_state(&state_path)
            .unwrap_or_else(|_| ampersona_core::state::PhaseState::new(persona.name.clone()));

        let pending = state
            .pending_transition
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no pending transition"))?;

        if pending.gate_id != gate_id {
            bail!("pending gate is '{}', not '{gate_id}'", pending.gate_id);
        }

        // Apply the pending transition
        let from_phase = pending.from_phase.clone();
        let to_phase = pending.to_phase.clone();
        let p_gate_id = pending.gate_id.clone();
        let metrics_hash = pending.metrics_hash.clone();

        state.current_phase = Some(to_phase.clone());
        state.state_rev += 1;
        state.updated_at = chrono::Utc::now();
        state.last_transition = Some(ampersona_core::state::TransitionRecord {
            gate_id: p_gate_id.clone(),
            from_phase: from_phase.clone(),
            to_phase: to_phase.clone(),
            at: chrono::Utc::now(),
            decision_id: format!("gate-{}", state.state_rev),
            metrics_hash: Some(metrics_hash.clone()),
            state_rev: state.state_rev,
        });
        state.pending_transition = None;

        let audit_entry = serde_json::json!({
            "event_type": "GateTransition",
            "gate_id": p_gate_id,
            "decision": "approved",
            "from_phase": from_phase,
            "to_phase": to_phase,
            "state_rev": state.state_rev,
            "metrics_hash": metrics_hash,
        });

        if let Ok(ref w) = writer {
            w.maybe_audit(persona.audit.as_ref(), "GateTransition", &audit_entry)?;
            w.write_state(&state)?;
        } else {
            let json = serde_json::to_string_pretty(&state)?;
            ampersona_engine::state::atomic::atomic_write(&state_path, json.as_bytes())?;
        }

        if json_out {
            let output = serde_json::json!({
                "gate_id": p_gate_id,
                "decision": "approved",
                "from_phase": from_phase,
                "to_phase": to_phase,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            eprintln!(
                "  approved: {} \u{2192} {}",
                from_phase.as_deref().unwrap_or("none"),
                to_phase
            );
        }
        return Ok(CmdExit::Ok);
    }

    if let Some(gate_id) = override_gate {
        let reason = reason.ok_or_else(|| anyhow::anyhow!("--reason required for override"))?;
        let approver =
            approver.ok_or_else(|| anyhow::anyhow!("--approver required for override"))?;

        let gate = persona
            .gates
            .as_ref()
            .and_then(|g| g.iter().find(|g| g.id == gate_id))
            .ok_or_else(|| anyhow::anyhow!("gate '{gate_id}' not found"))?;

        let state_path = file.replace(".json", ".state.json");
        let writer = ampersona_engine::state::writer::StateWriter::acquire(&state_path);
        let mut state = ampersona_engine::state::phase::load_state(&state_path)
            .unwrap_or_else(|_| ampersona_core::state::PhaseState::new(persona.name.clone()));

        // Phase match check: gate.from_phase must match current phase
        if gate.from_phase.as_deref() != state.current_phase.as_deref() {
            bail!(
                "override rejected: gate '{}' from_phase ({}) does not match current phase ({})",
                gate_id,
                gate.from_phase.as_deref().unwrap_or("null"),
                state.current_phase.as_deref().unwrap_or("null"),
            );
        }

        // Criteria check: if metrics provided, criteria must be failing
        if let Some(ref mf) = metrics_file {
            let mdata: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(mf)?)?;
            struct JsonMetricsOvr(serde_json::Value);
            impl ampersona_core::traits::MetricsProvider for JsonMetricsOvr {
                fn get_metric(
                    &self,
                    query: &ampersona_core::traits::MetricQuery,
                ) -> Result<ampersona_core::traits::MetricSample, ampersona_core::errors::MetricError>
                {
                    self.0
                        .get(&query.name)
                        .map(|v| ampersona_core::traits::MetricSample {
                            name: query.name.clone(),
                            value: v.clone(),
                            sampled_at: chrono::Utc::now(),
                        })
                        .ok_or(ampersona_core::errors::MetricError::NotFound(
                            query.name.clone(),
                        ))
                }
            }
            let m = JsonMetricsOvr(mdata);
            let evaluator = ampersona_engine::gates::evaluator::DefaultGateEvaluator;
            let (all_pass, _, _) = evaluator.evaluate_criteria(
                &gate.criteria,
                &m,
                gate.direction,
                gate.metrics_schema.as_ref(),
            );
            if all_pass {
                bail!(
                    "override rejected: gate '{}' criteria are already passing — no override needed",
                    gate_id,
                );
            }
        }

        let record = ampersona_engine::gates::override_gate::process_override(
            &ampersona_engine::gates::override_gate::OverrideRequest {
                gate_id: gate_id.clone(),
                direction: gate.direction,
                from_phase: state.current_phase.clone(),
                to_phase: gate.to_phase.clone(),
                reason: reason.clone(),
                approver: approver.clone(),
                state_rev: state.state_rev,
                metrics_snapshot: std::collections::HashMap::new(),
            },
        );

        state.current_phase = Some(record.to_phase.clone());
        state.state_rev += 1;
        state.updated_at = chrono::Utc::now();
        // Override clears any active overlay (ADR-010)
        state.active_overlay = None;

        // Audit the override
        let audit_entry = serde_json::json!({
            "event_type": "Override",
            "gate_id": record.gate_id,
            "direction": record.direction,
            "from_phase": record.from_phase,
            "to_phase": record.to_phase,
            "reason": reason,
            "approver": approver,
            "state_rev": state.state_rev,
        });

        if let Ok(ref w) = writer {
            w.maybe_audit(persona.audit.as_ref(), "Override", &audit_entry)?;
            w.write_state(&state)?;
        } else {
            let json = serde_json::to_string_pretty(&state)?;
            ampersona_engine::state::atomic::atomic_write(&state_path, json.as_bytes())?;
        }

        if !json_out {
            eprintln!(
                "  override: {} \u{2192} {} (by {approver})",
                record.from_phase.as_deref().unwrap_or("none"),
                record.to_phase
            );
        }
        println!("{}", serde_json::to_string_pretty(&record)?);
        return Ok(CmdExit::Ok);
    }

    if let Some(gate_id) = evaluate {
        let gates = persona
            .gates
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no gates defined"))?;

        let metrics_path =
            metrics_file.ok_or_else(|| anyhow::anyhow!("--metrics required for evaluate"))?;
        let metrics_data: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&metrics_path)?)?;

        struct JsonMetrics(serde_json::Value);
        impl ampersona_core::traits::MetricsProvider for JsonMetrics {
            fn get_metric(
                &self,
                query: &ampersona_core::traits::MetricQuery,
            ) -> Result<ampersona_core::traits::MetricSample, ampersona_core::errors::MetricError>
            {
                self.0
                    .get(&query.name)
                    .map(|v| ampersona_core::traits::MetricSample {
                        name: query.name.clone(),
                        value: v.clone(),
                        sampled_at: chrono::Utc::now(),
                    })
                    .ok_or(ampersona_core::errors::MetricError::NotFound(
                        query.name.clone(),
                    ))
            }
        }

        let metrics = JsonMetrics(metrics_data);
        let state_path = file.replace(".json", ".state.json");
        let writer = ampersona_engine::state::writer::StateWriter::acquire(&state_path);
        let mut state = ampersona_engine::state::phase::load_state(&state_path)
            .unwrap_or_else(|_| ampersona_core::state::PhaseState::new(persona.name.clone()));

        // Migrate legacy sidecar overlay into state (ADR-010)
        let sidecar_path = file.replace(".json", ".authority_overlay.json");
        if state.active_overlay.is_none() {
            if let Ok(sidecar_content) = std::fs::read_to_string(&sidecar_path) {
                if let Ok(overlay) = serde_json::from_str::<
                    ampersona_core::spec::authority::AuthorityOverlay,
                >(&sidecar_content)
                {
                    state.active_overlay = Some(overlay);
                    let _ = std::fs::remove_file(&sidecar_path);
                    if !json_out {
                        eprintln!("  migrated sidecar overlay to state");
                    }
                }
            }
        }

        // Enforce TTL on existing elevations
        ampersona_engine::state::elevation::enforce_ttl(&mut state);

        let evaluator = ampersona_engine::gates::evaluator::DefaultGateEvaluator;
        let result = evaluator.evaluate(gates, &state, &metrics);

        if let Some(record) = result {
            if record.gate_id == gate_id || gate_id == "*" {
                // Build audit entry once; each branch writes it exactly once.
                let audit_entry = serde_json::json!({
                    "event_type": "GateTransition",
                    "gate_id": record.gate_id,
                    "direction": record.direction,
                    "enforcement": record.enforcement,
                    "decision": record.decision,
                    "from_phase": record.from_phase,
                    "to_phase": record.to_phase,
                    "metrics_snapshot": record.metrics_snapshot,
                    "criteria_results": record.criteria_results,
                    "is_override": record.is_override,
                    "state_rev": record.state_rev,
                    "metrics_hash": record.metrics_hash,
                });

                // Helper: write one audit entry via writer or fallback
                let do_audit = |w: &Result<ampersona_engine::state::writer::StateWriter, _>,
                                entry: &serde_json::Value|
                 -> Result<()> {
                    if let Ok(ref w) = w {
                        w.maybe_audit(persona.audit.as_ref(), "GateTransition", entry)?;
                    } else {
                        let audit_path = file.replace(".json", ".audit.jsonl");
                        let _ =
                            ampersona_engine::state::audit_log::append_audit(&audit_path, entry);
                    }
                    Ok(())
                };

                // Write drift entry (always, regardless of decision)
                let drift_path = file.replace(".json", ".drift.jsonl");
                let _ = ampersona_engine::state::drift::append_drift(
                    &drift_path,
                    serde_json::json!(record.metrics_snapshot),
                );

                // Handle pending_human: write PendingTransition, don't apply
                if record.decision == "pending_human" {
                    state.pending_transition = Some(ampersona_core::state::PendingTransition {
                        gate_id: record.gate_id.clone(),
                        from_phase: record.from_phase.clone(),
                        to_phase: record.to_phase.clone(),
                        decision: record.decision.clone(),
                        metrics_hash: record.metrics_hash.clone(),
                        state_rev: state.state_rev,
                        created_at: chrono::Utc::now(),
                    });
                    state.updated_at = chrono::Utc::now();

                    do_audit(&writer, &audit_entry)?;
                    if let Ok(ref w) = writer {
                        w.write_state(&state)?;
                    } else {
                        let json = serde_json::to_string_pretty(&state)?;
                        ampersona_engine::state::atomic::atomic_write(
                            &state_path,
                            json.as_bytes(),
                        )?;
                    }

                    if !json_out {
                        eprintln!(
                            "  pending human approval: {} \u{2192} {} (use --approve {})",
                            record.from_phase.as_deref().unwrap_or("none"),
                            record.to_phase,
                            record.gate_id
                        );
                    }
                    println!("{}", serde_json::to_string_pretty(&record)?);
                    return Ok(CmdExit::Code(2));
                }

                // Handle quorum error
                if record.decision == "error_quorum_not_supported" {
                    do_audit(&writer, &audit_entry)?;
                    if !json_out {
                        eprintln!(
                            "  error: quorum approval not yet supported (gate {})",
                            record.gate_id
                        );
                    }
                    println!("{}", serde_json::to_string_pretty(&record)?);
                    return Ok(CmdExit::Code(1));
                }

                if record.enforcement == ampersona_core::types::GateEnforcement::Enforce
                    && record.decision == "transition"
                {
                    state.current_phase = Some(record.to_phase.clone());
                    state.state_rev += 1;
                    state.updated_at = chrono::Utc::now();
                    state.last_transition = Some(ampersona_core::state::TransitionRecord {
                        gate_id: record.gate_id.clone(),
                        from_phase: record.from_phase.clone(),
                        to_phase: record.to_phase.clone(),
                        at: chrono::Utc::now(),
                        decision_id: format!("gate-{}", state.state_rev),
                        metrics_hash: Some(record.metrics_hash.clone()),
                        state_rev: state.state_rev,
                    });
                    // Clear any pending transition since we're applying now
                    state.pending_transition = None;

                    // Apply authority overlay from on_pass (ADR-010: stored in state, not sidecar)
                    let previous_overlay = state.active_overlay.clone();
                    let fired_gate = gates.iter().find(|g| g.id == record.gate_id);
                    if let Some(gate) = fired_gate {
                        if let Some(effect) = &gate.on_pass {
                            state.active_overlay = effect.authority_overlay.clone();
                        } else {
                            state.active_overlay = None;
                        }
                    } else {
                        state.active_overlay = None;
                    }

                    do_audit(&writer, &audit_entry)?;

                    // Emit AuthorityOverlayChange audit event if overlay changed
                    let overlay_changed = match (&previous_overlay, &state.active_overlay) {
                        (None, None) => false,
                        (Some(_), None) | (None, Some(_)) => true,
                        (Some(a), Some(b)) => {
                            serde_json::to_string(a).ok() != serde_json::to_string(b).ok()
                        }
                    };
                    if overlay_changed {
                        let overlay_audit = serde_json::json!({
                            "event_type": "AuthorityOverlayChange",
                            "gate_id": record.gate_id,
                            "previous_overlay": previous_overlay,
                            "new_overlay": state.active_overlay,
                        });
                        if let Ok(ref w) = writer {
                            w.maybe_audit(
                                persona.audit.as_ref(),
                                "AuthorityOverlayChange",
                                &overlay_audit,
                            )?;
                        } else {
                            let audit_path = file.replace(".json", ".audit.jsonl");
                            let _ = ampersona_engine::state::audit_log::append_audit(
                                &audit_path,
                                &overlay_audit,
                            );
                        }
                        if !json_out {
                            if state.active_overlay.is_some() {
                                eprintln!("  authority overlay applied from gate on_pass");
                            } else {
                                eprintln!("  authority overlay cleared");
                            }
                        }
                    }
                    if let Ok(ref w) = writer {
                        w.write_state(&state)?;
                    } else {
                        let json = serde_json::to_string_pretty(&state)?;
                        ampersona_engine::state::atomic::atomic_write(
                            &state_path,
                            json.as_bytes(),
                        )?;
                    }
                    if !json_out {
                        eprintln!(
                            "  transition: {} \u{2192} {}",
                            record.from_phase.as_deref().unwrap_or("none"),
                            record.to_phase
                        );
                    }
                } else if record.decision == "observed" {
                    do_audit(&writer, &audit_entry)?;
                    if !json_out {
                        eprintln!(
                            "  observed (not applied): {} \u{2192} {}",
                            record.from_phase.as_deref().unwrap_or("none"),
                            record.to_phase
                        );
                    }
                }
                println!("{}", serde_json::to_string_pretty(&record)?);
                return Ok(CmdExit::Ok);
            } else {
                if !json_out {
                    eprintln!(
                        "  gate '{gate_id}' did not fire (another gate matched: {})",
                        record.gate_id
                    );
                }
            }
        }

        // No gate fired (or requested gate didn't match).
        // If a specific gate was requested and --json, produce diagnostic.
        if json_out && gate_id != "*" {
            if let Some(gate) = gates.iter().find(|g| g.id == gate_id) {
                let diagnostic = diagnose_gate(gate, &metrics);
                println!("{}", serde_json::to_string_pretty(&diagnostic)?);
            } else {
                let diagnostic = serde_json::json!({
                    "gate_id": gate_id,
                    "decision": "not_found",
                    "reason": format!("gate '{gate_id}' not defined"),
                });
                println!("{}", serde_json::to_string_pretty(&diagnostic)?);
            }
        } else if !json_out {
            eprintln!("  no gate fired");
        }
        return Ok(CmdExit::Code(1));
    }

    bail!("specify --evaluate or --override");
}

/// Produce diagnostic JSON for a gate whose criteria failed.
fn diagnose_gate(
    gate: &ampersona_core::spec::gates::Gate,
    metrics: &dyn ampersona_core::traits::MetricsProvider,
) -> serde_json::Value {
    let mut criteria_results = Vec::new();
    for criterion in &gate.criteria {
        let query = ampersona_core::traits::MetricQuery {
            name: criterion.metric.clone(),
            window: None,
        };
        let (actual, pass) = match metrics.get_metric(&query) {
            Ok(sample) => {
                let pass = compare_criterion(&criterion.op, &sample.value, &criterion.value);
                (sample.value, pass)
            }
            Err(_) => (serde_json::Value::Null, false),
        };
        criteria_results.push(serde_json::json!({
            "metric": criterion.metric,
            "op": criterion.op,
            "value": criterion.value,
            "actual": actual,
            "pass": pass,
        }));
    }
    serde_json::json!({
        "gate_id": gate.id,
        "decision": "no_match",
        "reason": "criteria not met",
        "criteria_results": criteria_results,
    })
}

fn compare_criterion(
    op: &ampersona_core::types::CriterionOp,
    actual: &serde_json::Value,
    expected: &serde_json::Value,
) -> bool {
    use ampersona_core::types::CriterionOp;
    match op {
        CriterionOp::Eq => actual == expected,
        CriterionOp::Neq => actual != expected,
        CriterionOp::Gt => cmp_num(actual, expected).is_some_and(|c| c > 0),
        CriterionOp::Gte => cmp_num(actual, expected).is_some_and(|c| c >= 0),
        CriterionOp::Lt => cmp_num(actual, expected).is_some_and(|c| c < 0),
        CriterionOp::Lte => cmp_num(actual, expected).is_some_and(|c| c <= 0),
    }
}

fn cmp_num(a: &serde_json::Value, b: &serde_json::Value) -> Option<i8> {
    let a_f = a.as_f64()?;
    let b_f = b.as_f64()?;
    if a_f > b_f {
        Some(1)
    } else if a_f < b_f {
        Some(-1)
    } else {
        Some(0)
    }
}

fn cmd_sign(file: &str, key_path: &str, key_id: &str) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    let mut data: serde_json::Value = serde_json::from_str(&content)?;

    let key_bytes =
        std::fs::read(key_path).map_err(|e| anyhow::anyhow!("cannot read key {key_path}: {e}"))?;
    let key_array: [u8; 32] = key_bytes
        .get(..32)
        .ok_or_else(|| anyhow::anyhow!("key must be at least 32 bytes"))?
        .try_into()
        .unwrap();
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&key_array);

    ampersona_sign::sign::sign_persona(&mut data, &signing_key, key_id, "cli")?;

    let json = serde_json::to_string_pretty(&data)?;
    std::fs::write(file, &json)?;
    eprintln!("  signed {file} (key_id: {key_id})");
    Ok(())
}

fn cmd_verify(file: &str, pubkey_path: &str) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    let data: serde_json::Value = serde_json::from_str(&content)?;

    let key_bytes = std::fs::read(pubkey_path)
        .map_err(|e| anyhow::anyhow!("cannot read pubkey {pubkey_path}: {e}"))?;
    let key_array: [u8; 32] = key_bytes
        .get(..32)
        .ok_or_else(|| anyhow::anyhow!("pubkey must be at least 32 bytes"))?
        .try_into()
        .unwrap();
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&key_array)
        .map_err(|e| anyhow::anyhow!("invalid pubkey: {e}"))?;

    let valid = ampersona_sign::verify::verify_persona(&data, &verifying_key)?;
    if valid {
        eprintln!("  signature valid");
    } else {
        bail!("signature verification failed");
    }
    Ok(())
}

struct AuditOpts {
    file: String,
    verify: bool,
    from: Option<u64>,
    checkpoint_create: bool,
    checkpoint_verify: bool,
    checkpoint_path: Option<String>,
    sign_key: Option<String>,
    sign_key_id: String,
    verify_key: Option<String>,
    json_out: bool,
}

fn cmd_audit(opts: AuditOpts) -> CmdExit {
    let AuditOpts {
        file,
        verify,
        from,
        checkpoint_create,
        checkpoint_verify,
        checkpoint_path,
        sign_key,
        sign_key_id,
        verify_key,
        json_out,
    } = opts;
    let audit_path = file.replace(".json", ".audit.jsonl");

    // Handle checkpoint create
    if checkpoint_create {
        let cp_path = checkpoint_path.unwrap_or_else(|| file.replace(".json", ".checkpoint.json"));
        if !std::path::Path::new(&audit_path).exists() {
            return CmdExit::Err(anyhow::anyhow!("no audit log at {audit_path}"));
        }
        match ampersona_engine::state::audit_log::create_checkpoint(&audit_path, &cp_path) {
            Ok(mut checkpoint) => {
                // Optionally sign the checkpoint
                if let Some(ref key_path) = sign_key {
                    match sign_checkpoint(&mut checkpoint, key_path, &sign_key_id) {
                        Ok(()) => {
                            // Re-write with signature
                            let json = serde_json::to_string_pretty(&checkpoint).unwrap();
                            if let Err(e) = std::fs::write(&cp_path, json) {
                                return CmdExit::Err(e.into());
                            }
                        }
                        Err(e) => return CmdExit::Err(e),
                    }
                }
                if json_out {
                    println!("{}", serde_json::to_string_pretty(&checkpoint).unwrap());
                } else {
                    eprintln!("  checkpoint created at {cp_path}");
                }
                return CmdExit::Ok;
            }
            Err(e) => return CmdExit::Err(e),
        }
    }

    // Handle checkpoint verify
    if checkpoint_verify {
        let cp_path = checkpoint_path.unwrap_or_else(|| file.replace(".json", ".checkpoint.json"));
        if !std::path::Path::new(&audit_path).exists() {
            return CmdExit::Err(anyhow::anyhow!("no audit log at {audit_path}"));
        }
        if !std::path::Path::new(&cp_path).exists() {
            return CmdExit::Err(anyhow::anyhow!("no checkpoint at {cp_path}"));
        }

        // Verify signature if public key provided
        if let Some(ref pubkey_path) = verify_key {
            match verify_checkpoint_signature(&cp_path, pubkey_path) {
                Ok(true) => {
                    if !json_out {
                        eprintln!("  checkpoint signature valid");
                    }
                }
                Ok(false) => {
                    if json_out {
                        let output = serde_json::json!({
                            "valid": false,
                            "error": "checkpoint signature verification failed",
                        });
                        println!("{}", serde_json::to_string_pretty(&output).unwrap());
                    } else {
                        eprintln!("  checkpoint signature INVALID");
                    }
                    return CmdExit::Code(1);
                }
                Err(e) => return CmdExit::Err(e),
            }
        }

        match ampersona_engine::state::audit_log::verify_checkpoint(&audit_path, &cp_path) {
            Ok(true) => {
                if json_out {
                    let output = serde_json::json!({
                        "valid": true,
                        "checkpoint": cp_path,
                        "audit_path": audit_path,
                    });
                    println!("{}", serde_json::to_string_pretty(&output).unwrap());
                } else {
                    eprintln!("  checkpoint valid");
                }
                CmdExit::Ok
            }
            Ok(false) => {
                if json_out {
                    let output = serde_json::json!({
                        "valid": false,
                        "checkpoint": cp_path,
                        "audit_path": audit_path,
                    });
                    println!("{}", serde_json::to_string_pretty(&output).unwrap());
                } else {
                    eprintln!("  checkpoint INVALID");
                }
                CmdExit::Code(1)
            }
            Err(e) => CmdExit::Err(e),
        }
    } else {
        // Standard --verify
        if !verify {
            return CmdExit::Err(anyhow::anyhow!(
                "specify --verify, --checkpoint-create, or --checkpoint-verify"
            ));
        }
        if !std::path::Path::new(&audit_path).exists() {
            if json_out {
                let output = serde_json::json!({
                    "valid": true,
                    "entries": 0,
                    "audit_path": audit_path,
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                eprintln!("  no audit log found at {audit_path}");
            }
            return CmdExit::Ok;
        }
        let from_entry = from.unwrap_or(0);
        match ampersona_engine::state::audit_log::verify_chain_from(&audit_path, from_entry) {
            Ok(count) => {
                if json_out {
                    let mut output = serde_json::json!({
                        "valid": true,
                        "entries": count,
                        "audit_path": audit_path,
                    });
                    if from_entry > 0 {
                        output["from_entry"] = serde_json::json!(from_entry);
                    }

                    // state_rev consistency check
                    let state_path = file.replace(".json", ".state.json");
                    if let Ok(state) = ampersona_engine::state::phase::load_state(&state_path) {
                        if std::path::Path::new(&audit_path).exists() {
                            let transitions =
                                ampersona_engine::state::audit_log::count_gate_transitions(
                                    &audit_path,
                                )
                                .unwrap_or(0);
                            // state_rev can be > transitions+1 if pending/approve both increment
                            // but state_rev > gate_transitions + pending_count is suspicious
                            let consistent = state.state_rev <= transitions + 1
                                || state.last_transition.is_some();
                            output["state_rev_check"] = serde_json::json!({
                                "state_rev": state.state_rev,
                                "gate_transitions": transitions,
                                "consistent": consistent,
                            });
                            if !consistent {
                                eprintln!(
                                    "  warn: state_rev ({}) exceeds gate_transition count ({}) + 1",
                                    state.state_rev, transitions
                                );
                            }
                        }
                    }

                    println!("{}", serde_json::to_string_pretty(&output).unwrap());
                } else {
                    if from_entry > 0 {
                        eprintln!("  audit chain valid ({count} entries, verified from entry {from_entry})");
                    } else {
                        eprintln!("  audit chain valid ({count} entries)");
                    }
                }
                CmdExit::Ok
            }
            Err(e) => {
                let msg = format!("{e:#}");
                if json_out {
                    let output = serde_json::json!({
                        "valid": false,
                        "error": msg,
                        "audit_path": audit_path,
                    });
                    println!("{}", serde_json::to_string_pretty(&output).unwrap());
                } else {
                    eprintln!("  audit chain INVALID: {msg}");
                }
                CmdExit::Code(1)
            }
        }
    }
}

/// Sign a checkpoint JSON value with ed25519.
fn sign_checkpoint(checkpoint: &mut serde_json::Value, key_path: &str, key_id: &str) -> Result<()> {
    let key_bytes =
        std::fs::read(key_path).map_err(|e| anyhow::anyhow!("cannot read key {key_path}: {e}"))?;
    let key_array: [u8; 32] = key_bytes
        .get(..32)
        .ok_or_else(|| anyhow::anyhow!("key must be at least 32 bytes"))?
        .try_into()
        .unwrap();
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&key_array);

    // Sign the canonical checkpoint JSON (without any existing signature)
    let mut signable = checkpoint.clone();
    if let Some(obj) = signable.as_object_mut() {
        obj.remove("signature");
    }
    let canonical = serde_json::to_string(&signable)?;
    use ed25519_dalek::Signer;
    let sig = signing_key.sign(canonical.as_bytes());

    let sig_hex: String = sig.to_bytes().iter().map(|b| format!("{b:02x}")).collect();
    if let Some(obj) = checkpoint.as_object_mut() {
        obj.insert(
            "signature".into(),
            serde_json::json!({
                "key_id": key_id,
                "algorithm": "ed25519",
                "value": sig_hex,
            }),
        );
    }
    Ok(())
}

/// Verify a signed checkpoint file.
fn verify_checkpoint_signature(checkpoint_path: &str, pubkey_path: &str) -> Result<bool> {
    let content = std::fs::read_to_string(checkpoint_path)?;
    let checkpoint: serde_json::Value = serde_json::from_str(&content)?;

    let sig_obj = checkpoint
        .get("signature")
        .ok_or_else(|| anyhow::anyhow!("checkpoint has no signature"))?;
    let sig_hex = sig_obj
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("invalid signature format"))?;
    let sig_bytes: Vec<u8> = (0..sig_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&sig_hex[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|e| anyhow::anyhow!("invalid hex in signature: {e}"))?;
    let sig = ed25519_dalek::Signature::from_slice(&sig_bytes)
        .map_err(|e| anyhow::anyhow!("invalid signature: {e}"))?;

    let key_bytes = std::fs::read(pubkey_path)
        .map_err(|e| anyhow::anyhow!("cannot read pubkey {pubkey_path}: {e}"))?;
    let key_array: [u8; 32] = key_bytes
        .get(..32)
        .ok_or_else(|| anyhow::anyhow!("pubkey must be at least 32 bytes"))?
        .try_into()
        .unwrap();
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&key_array)
        .map_err(|e| anyhow::anyhow!("invalid pubkey: {e}"))?;

    // Reconstruct canonical form without signature
    let mut signable = checkpoint.clone();
    if let Some(obj) = signable.as_object_mut() {
        obj.remove("signature");
    }
    let canonical = serde_json::to_string(&signable)?;

    use ed25519_dalek::Verifier;
    Ok(verifying_key.verify(canonical.as_bytes(), &sig).is_ok())
}

fn cmd_compose(base_path: &str, overlay_path: &str) -> Result<()> {
    let base = ampersona_core::prompt::load_persona(base_path)?;
    let overlay = ampersona_core::prompt::load_persona(overlay_path)?;
    let merged = ampersona_core::compose::merge_personas(&base, &overlay);
    println!("{}", serde_json::to_string_pretty(&merged)?);
    Ok(())
}

fn cmd_diff(a_path: &str, b_path: &str) -> Result<()> {
    let a = ampersona_core::prompt::load_persona(a_path)?;
    let b = ampersona_core::prompt::load_persona(b_path)?;

    fn diff_values(path: &str, a: &serde_json::Value, b: &serde_json::Value) {
        if a == b {
            return;
        }
        match (a, b) {
            (serde_json::Value::Object(ao), serde_json::Value::Object(bo)) => {
                let all_keys: std::collections::BTreeSet<_> = ao.keys().chain(bo.keys()).collect();
                for key in all_keys {
                    let subpath = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    match (ao.get(key), bo.get(key)) {
                        (Some(av), Some(bv)) => diff_values(&subpath, av, bv),
                        (Some(av), None) => println!("- {subpath}: {av}"),
                        (None, Some(bv)) => println!("+ {subpath}: {bv}"),
                        (None, None) => {}
                    }
                }
            }
            _ => {
                println!("- {path}: {a}");
                println!("+ {path}: {b}");
            }
        }
    }

    diff_values("", &a, &b);
    Ok(())
}

fn cmd_import(file: &str, from: &str) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    let data: serde_json::Value = serde_json::from_str(&content)?;
    let persona = match from {
        "aieos" => ampersona_engine::convert::aieos::import_aieos(&data)?,
        "zeroclaw" => ampersona_engine::convert::zeroclaw::import_zeroclaw(&data)?,
        _ => bail!("import from '{from}' not supported (use: aieos, zeroclaw)"),
    };
    println!("{}", serde_json::to_string_pretty(&persona)?);
    Ok(())
}

fn cmd_export(file: &str, to: &str) -> Result<()> {
    let data = read_persona(file)?;
    let exported = match to {
        "aieos" => ampersona_engine::convert::aieos::export_aieos(&data)?,
        "zeroclaw-config" | "zeroclaw" => {
            ampersona_engine::convert::zeroclaw::export_zeroclaw(&data)?
        }
        _ => bail!("export to '{to}' not supported (use: aieos, zeroclaw-config)"),
    };
    println!("{}", serde_json::to_string_pretty(&exported)?);
    Ok(())
}

fn cmd_fleet(
    dir: &str,
    status: bool,
    check: bool,
    json_out: bool,
    apply_overlay: Option<String>,
) -> Result<()> {
    let entries = std::fs::read_dir(dir)?;
    let mut files: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .filter(|e| !e.file_name().to_string_lossy().ends_with(".state.json"))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();
    files.sort();

    if status {
        println!(
            "{:<30}  {:<10}  {:<12}  {:<10}",
            "FILE", "NAME", "AUTONOMY", "PHASE"
        );
        println!(
            "{:<30}  {:<10}  {:<12}  {:<10}",
            "-".repeat(30),
            "-".repeat(10),
            "-".repeat(12),
            "-".repeat(10)
        );
        for file in &files {
            let data = ampersona_core::prompt::load_persona(file)?;
            let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("-");
            let autonomy = data
                .pointer("/authority/autonomy")
                .and_then(|v| v.as_str())
                .unwrap_or("-");
            let state_path = file.replace(".json", ".state.json");
            let phase = ampersona_engine::state::phase::load_state(&state_path)
                .ok()
                .and_then(|s| s.current_phase)
                .unwrap_or_else(|| "-".into());
            let fname = std::path::Path::new(file)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            println!("{fname:<30}  {name:<10}  {autonomy:<12}  {phase:<10}");
        }
        return Ok(());
    }

    if check {
        let mut reports = Vec::new();
        for file in &files {
            let content = std::fs::read_to_string(file)?;
            let data: serde_json::Value = serde_json::from_str(&content)?;
            let report = ampersona_core::schema::check(&data, file, false);
            if !json_out {
                if report.pass {
                    eprintln!("  ok  {file}");
                } else {
                    eprintln!("  FAIL {file}");
                    for e in &report.errors {
                        eprintln!("    {}: {}", e.code, e.message);
                    }
                }
            }
            reports.push(report);
        }
        if json_out {
            println!("{}", serde_json::to_string_pretty(&reports)?);
        }
        return Ok(());
    }

    if let Some(overlay_path) = apply_overlay {
        let overlay = ampersona_core::prompt::load_persona(&overlay_path)?;
        for file in &files {
            let base = ampersona_core::prompt::load_persona(file)?;
            let merged = ampersona_core::compose::merge_personas(&base, &overlay);
            let json = serde_json::to_string_pretty(&merged)?;
            std::fs::write(file, json)?;
            eprintln!("  applied overlay to {file}");
        }
        return Ok(());
    }

    bail!("specify --status, --check, or --apply-overlay");
}
