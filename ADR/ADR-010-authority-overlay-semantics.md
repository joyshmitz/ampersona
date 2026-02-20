# ADR-010: Authority Overlay Semantics

**Status:** Accepted
**Date:** 2026-02-20
**Supersedes:** (implicit meet-semilattice behavior)

## Context

Gate transitions can carry an `on_pass.authority_overlay` that modifies the agent's
effective authority for the duration of the target phase. The original implementation
merged overlays into `resolve_authority` as an additional layer, applying the same
meet-semilattice rules (deny=union, allow=intersection, autonomy=min, limits=min).

**Problem:** A meet-semilattice merge is mathematically incapable of expanding
permissions. Since every layer can only restrict (intersection, minimum), an overlay
with `autonomy: "full"` applied to a `supervised` persona yields
`min(supervised, full) = supervised`. Similarly, adding `deploy` to the allow list
via overlay results in `{read} ∩ {read, deploy} = {read}`.

This means all three consumer example personas (`quiet_stone_v1`, `zeroclaw_agent`,
`odoov19_quality`) have promote gates with authority overlays that silently do nothing.

**Proof:**

```
resolve_authority([persona, overlay])
  autonomy = min(persona.autonomy, overlay.autonomy)
           = min(supervised, full)
           = supervised  ← overlay ignored

  allowed  = persona.allow ∩ overlay.allow
           = {read} ∩ {read, deploy}
           = {read}      ← deploy dropped
```

## Decision

Authority overlay uses **patch-replace** semantics, not layered merge.

### New type: `AuthorityOverlay`

All fields are `Option`. Only present fields are applied:

```rust
pub struct AuthorityOverlay {
    pub autonomy: Option<AutonomyLevel>,
    pub scope: Option<Scope>,
    pub actions: Option<Actions>,
    pub limits: Option<Limits>,
}
```

### New function: `apply_overlay(base, overlay) -> ResolvedAuthority`

```
apply_overlay(resolved, overlay):
  if overlay.autonomy.is_some():
    result.autonomy = overlay.autonomy          // REPLACE
  if overlay.actions.allow.is_some():
    result.allowed = overlay.allow \ denied      // REPLACE, minus deny
  if overlay.actions.deny.is_some():
    result.denied ∪= overlay.deny               // UNION (additive)
    result.allowed \= overlay.deny              // remove from allowed
  if overlay.scope.is_some():
    result.scope = overlay.scope                // REPLACE
  if overlay.limits.is_some():
    result.limits = overlay.limits              // REPLACE
```

### Invariant preserved

**Deny preservation:** `deny(persona) ⊆ deny(effective)`.
Explicit deny entries from the persona are never removed by overlay.
Overlay can add new deny entries but cannot remove existing ones.

### Lifecycle

- Gate fires with `on_pass.authority_overlay` → stored in `PhaseState.active_overlay`
- Overlay cleared when: agent transitions to another phase, or override occurs
- Second overlay replaces first completely (no stacking)
- Applied after `resolve_with_elevations()`, before policy check

## Consequences

- Promote gates can now meaningfully expand agent authority
- Demote gates can set restrictive overlays (e.g., `autonomy: "readonly"`)
- Overlay is a post-resolution patch, not a merge layer
- `AuthorityOverlayChange` audit event emitted on every overlay change
- THEORY.md updated with overlay in Reset function
- Sidecar `.authority_overlay.json` files migrated to `PhaseState.active_overlay`
- Backward compatible: existing JSON `{"autonomy":"full"}` parses as
  `AuthorityOverlay { autonomy: Some(Full), .. }`
