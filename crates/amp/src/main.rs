#![forbid(unsafe_code)]

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
        #[arg(long, name = "override")]
        override_gate: Option<String>,

        /// Reason for override.
        #[arg(long)]
        reason: Option<String>,

        /// Approver for override.
        #[arg(long)]
        approver: Option<String>,
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
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

        Cmd::Authority { file, check } => cmd_authority(&file, &check),

        Cmd::Elevate {
            file,
            elevation,
            reason,
        } => cmd_elevate(&file, &elevation, &reason),

        Cmd::Gate {
            file,
            evaluate,
            metrics,
            override_gate,
            reason,
            approver,
        } => cmd_gate(&file, evaluate, metrics, override_gate, reason, approver),

        Cmd::Sign { file, key, key_id } => cmd_sign(&file, &key, &key_id),

        Cmd::Verify { file, pubkey } => cmd_verify(&file, &pubkey),

        Cmd::Audit { file, verify } => cmd_audit(&file, verify),

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

fn cmd_authority(file: &str, action: &str) -> Result<()> {
    let data = read_persona(file)?;

    // Parse the persona to get authority
    let persona: ampersona_core::spec::Persona = serde_json::from_value(data.clone())?;

    let decision = if let Some(authority) = &persona.authority {
        // Build authority layers: workspace defaults (lowest) → persona (highest)
        let mut layers: Vec<&ampersona_core::spec::authority::Authority> = Vec::new();
        let workspace_defaults = ampersona_engine::policy::precedence::load_workspace_defaults();
        if let Some(ref wd) = workspace_defaults {
            layers.push(wd);
        }
        layers.push(authority);

        // Load state to check active elevations
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

        let checker = ampersona_engine::policy::checker::DefaultPolicyChecker;

        use ampersona_core::traits::AuthorityEnforcer;
        let req = ampersona_core::traits::PolicyRequest {
            action: Some(action.parse().unwrap_or_else(|_| {
                ampersona_core::actions::ActionId::Custom {
                    vendor: "_unknown".into(),
                    action: action.into(),
                }
            })),
            path: None,
            context: std::collections::HashMap::new(),
        };
        checker.evaluate(&req, &resolved)?
    } else {
        ampersona_core::errors::PolicyDecision::Deny {
            reason: "no authority section defined".to_string(),
        }
    };

    println!("{decision}");
    Ok(())
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
    let mut state = ampersona_engine::state::phase::load_state(&state_path)
        .unwrap_or_else(|_| ampersona_core::state::PhaseState::new(persona.name.clone()));

    ampersona_engine::state::elevation::activate(
        &mut state,
        elevation_id,
        elev.ttl_seconds as i64,
        reason,
        "cli",
    );
    state.state_rev += 1;
    state.updated_at = chrono::Utc::now();

    ampersona_engine::state::phase::save_state(&state_path, &state)?;
    eprintln!(
        "  elevation '{elevation_id}' activated (TTL: {}s)",
        elev.ttl_seconds
    );
    Ok(())
}

fn cmd_gate(
    file: &str,
    evaluate: Option<String>,
    metrics_file: Option<String>,
    override_gate: Option<String>,
    reason: Option<String>,
    approver: Option<String>,
) -> Result<()> {
    let data = read_persona(file)?;
    let persona: ampersona_core::spec::Persona = serde_json::from_value(data)?;

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
        let mut state = ampersona_engine::state::phase::load_state(&state_path)
            .unwrap_or_else(|_| ampersona_core::state::PhaseState::new(persona.name.clone()));

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
        ampersona_engine::state::phase::save_state(&state_path, &state)?;

        eprintln!(
            "  override: {} → {} (by {approver})",
            record.from_phase.as_deref().unwrap_or("none"),
            record.to_phase
        );
        println!("{}", serde_json::to_string_pretty(&record)?);
        return Ok(());
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

        // Build a simple metrics provider from JSON
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
        let mut state = ampersona_engine::state::phase::load_state(&state_path)
            .unwrap_or_else(|_| ampersona_core::state::PhaseState::new(persona.name.clone()));

        let evaluator = ampersona_engine::gates::evaluator::DefaultGateEvaluator;
        let result = evaluator.evaluate(gates, &state, &metrics);

        if let Some(record) = result {
            if record.gate_id == gate_id || gate_id == "*" {
                // Write audit entry
                let audit_path = file.replace(".json", ".audit.jsonl");
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
                let _ = ampersona_engine::state::audit_log::append_audit(&audit_path, &audit_entry);

                // Write drift entry
                let drift_path = file.replace(".json", ".drift.jsonl");
                let _ = ampersona_engine::state::drift::append_drift(
                    &drift_path,
                    serde_json::json!(record.metrics_snapshot),
                );

                if record.enforcement == ampersona_core::types::GateEnforcement::Enforce {
                    state.current_phase = Some(record.to_phase.clone());
                    state.state_rev += 1;
                    state.updated_at = chrono::Utc::now();
                    state.last_transition = Some(ampersona_core::state::TransitionRecord {
                        gate_id: record.gate_id.clone(),
                        from_phase: record.from_phase.clone(),
                        to_phase: record.to_phase.clone(),
                        at: chrono::Utc::now(),
                        decision_id: format!("gate-{}", state.state_rev),
                    });

                    // Apply authority overlay from on_pass if present
                    let fired_gate = gates.iter().find(|g| g.id == record.gate_id);
                    if let Some(gate) = fired_gate {
                        if let Some(effect) = &gate.on_pass {
                            if let Some(overlay) = &effect.authority_overlay {
                                // Write overlay alongside state for consumer use
                                let overlay_path = file.replace(".json", ".authority_overlay.json");
                                let overlay_json = serde_json::to_string_pretty(overlay)?;
                                std::fs::write(&overlay_path, overlay_json)?;
                                eprintln!("  authority overlay written to {overlay_path}");
                            }
                        }
                    }

                    ampersona_engine::state::phase::save_state(&state_path, &state)?;
                    eprintln!(
                        "  transition: {} → {}",
                        record.from_phase.as_deref().unwrap_or("none"),
                        record.to_phase
                    );
                } else {
                    eprintln!(
                        "  observed (not applied): {} → {}",
                        record.from_phase.as_deref().unwrap_or("none"),
                        record.to_phase
                    );
                }
                println!("{}", serde_json::to_string_pretty(&record)?);
            } else {
                eprintln!(
                    "  gate '{gate_id}' did not fire (another gate matched: {})",
                    record.gate_id
                );
            }
        } else {
            eprintln!("  no gate fired");
        }
        return Ok(());
    }

    bail!("specify --evaluate or --override");
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

fn cmd_audit(file: &str, verify: bool) -> Result<()> {
    if !verify {
        bail!("specify --verify");
    }
    let audit_path = file.replace(".json", ".audit.jsonl");
    if !std::path::Path::new(&audit_path).exists() {
        eprintln!("  no audit log found at {audit_path}");
        return Ok(());
    }
    let count = ampersona_engine::state::audit_log::verify_chain(&audit_path)?;
    eprintln!("  audit chain valid ({count} entries)");
    Ok(())
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
