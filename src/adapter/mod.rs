pub mod claude;
pub mod codex;

use crate::event::AgentEventKind;

pub(crate) fn json_str<'a>(val: &'a serde_json::Value, key: &str) -> &'a str {
    val.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

pub(crate) fn optional_str(val: &serde_json::Value, key: &str) -> Option<String> {
    let s = json_str(val, key);
    if s.is_empty() { None } else { Some(s.into()) }
}

/// Binding between an upstream agent-side hook trigger (as it appears in the
/// agent's `settings.json`) and the internal `AgentEventKind` the sidebar
/// produces once the hook fires.
///
/// Each adapter exposes its full `HOOK_REGISTRATIONS` table so install
/// wizards, README snippets, setup commands, and docs can all be generated
/// from a single source of truth. The `kind` field is a compile-time enum,
/// not a string, so typos cannot creep in. Drift between the table and the
/// adapter's `parse()` match arms is caught by the tests in `claude.rs` /
/// `codex.rs` via [`assert_table_drift_free`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookRegistration {
    /// Trigger name in the agent's settings.json (e.g. `"SessionStart"`,
    /// `"PostToolUse"`).
    pub trigger: &'static str,
    /// Optional matcher value. `None` means "register with an empty matcher"
    /// (catches all). `Some("startup|resume")` etc. captures a specific filter.
    pub matcher: Option<&'static str>,
    /// Internal event this registration produces.
    pub kind: AgentEventKind,
}

#[cfg(test)]
pub(crate) fn minimal_payload(kind: AgentEventKind) -> serde_json::Value {
    use serde_json::json;
    match kind {
        AgentEventKind::ActivityLog => json!({"tool_name": "Read"}),
        AgentEventKind::SubagentStart | AgentEventKind::SubagentStop => {
            json!({"agent_type": "Explore"})
        }
        _ => json!({}),
    }
}

#[cfg(test)]
pub(crate) fn assert_table_drift_free(agent: &str, table: &[HookRegistration]) {
    use crate::event::resolve_adapter;
    let adapter = resolve_adapter(agent).expect("adapter should exist");

    // Table → parse: every registration must be accepted by `parse()` and
    // produce an `AgentEvent` whose kind matches the registration.
    for reg in table {
        let event_name = reg.kind.external_name();
        let payload = minimal_payload(reg.kind);
        let produced = adapter.parse(event_name, &payload).unwrap_or_else(|| {
            panic!(
                "{agent}: HOOK_REGISTRATIONS lists {:?} but parse() returned None — parse arm missing",
                reg.kind
            )
        });
        assert_eq!(
            produced.kind(),
            reg.kind,
            "{agent}: table declares {:?} but parse() produced {:?}",
            reg.kind,
            produced.kind()
        );
    }

    // Parse → table: every kind `parse()` accepts must appear in the table.
    // Catches "added parse arm, forgot to update HOOK_REGISTRATIONS".
    for kind in AgentEventKind::ALL {
        let accepted = adapter
            .parse(kind.external_name(), &minimal_payload(*kind))
            .is_some();
        let in_table = table.iter().any(|r| r.kind == *kind);
        assert!(
            !accepted || in_table,
            "{agent}: parse() accepts {:?} but HOOK_REGISTRATIONS does not list it — add it to the table",
            kind
        );
    }
}
