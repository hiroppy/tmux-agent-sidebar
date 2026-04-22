/// Append a subagent entry to the comma-separated `@pane_subagents` list.
///
/// Format: each entry is `agent_type:agent_id`. The id suffix lets
/// `remove_subagent` match the exact instance on stop, and also lets the
/// UI render a stable `#<id-prefix>` tag that does not shift when siblings
/// stop.
pub(in crate::cli::hook) fn append_subagent(
    current: &str,
    agent_type: &str,
    agent_id: &str,
) -> String {
    let entry = format!("{}:{}", agent_type, agent_id);
    if current.is_empty() {
        entry
    } else {
        format!("{},{}", current, entry)
    }
}

/// Remove the entry with the given `agent_id` from the comma-separated list.
/// Returns `None` if `agent_id` is not present, `Some(new_list)` otherwise
/// (empty string if the list becomes empty).
pub(in crate::cli::hook) fn remove_subagent(current: &str, agent_id: &str) -> Option<String> {
    if current.is_empty() || agent_id.is_empty() {
        return None;
    }
    let needle = format!(":{}", agent_id);
    let items: Vec<&str> = current.split(',').collect();
    let idx = items.iter().position(|entry| entry.ends_with(&needle))?;
    let filtered: Vec<&str> = items
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != idx)
        .map(|(_, s)| *s)
        .collect();
    Some(filtered.join(","))
}

#[cfg(test)]
mod tests {
    use super::super::location::should_update_cwd;
    use super::*;

    #[test]
    fn append_subagent_to_empty() {
        assert_eq!(append_subagent("", "Explore", "sub-1"), "Explore:sub-1");
    }

    #[test]
    fn append_subagent_to_existing() {
        assert_eq!(
            append_subagent("Explore:sub-1", "Plan", "sub-2"),
            "Explore:sub-1,Plan:sub-2"
        );
    }

    #[test]
    fn append_subagent_same_type_parallel() {
        // Two Explore subagents running in parallel must be stored as
        // distinct entries — the ids disambiguate them.
        let list = append_subagent("Explore:sub-1", "Explore", "sub-2");
        assert_eq!(list, "Explore:sub-1,Explore:sub-2");
    }

    #[test]
    fn remove_subagent_empty_list() {
        assert_eq!(remove_subagent("", "sub-1"), None);
    }

    #[test]
    fn remove_subagent_empty_id_is_noop() {
        assert_eq!(remove_subagent("Explore:sub-1", ""), None);
    }

    #[test]
    fn remove_subagent_id_not_found() {
        assert_eq!(remove_subagent("Explore:sub-1,Plan:sub-2", "sub-9"), None);
    }

    #[test]
    fn remove_subagent_single_item() {
        assert_eq!(remove_subagent("Explore:sub-1", "sub-1"), Some("".into()));
    }

    #[test]
    fn remove_subagent_first_item() {
        assert_eq!(
            remove_subagent("Explore:sub-1,Plan:sub-2", "sub-1"),
            Some("Plan:sub-2".into())
        );
    }

    #[test]
    fn remove_subagent_middle_item() {
        assert_eq!(
            remove_subagent("Explore:sub-1,Plan:sub-2,Bash:sub-3", "sub-2"),
            Some("Explore:sub-1,Bash:sub-3".into())
        );
    }

    #[test]
    fn remove_subagent_last_item() {
        assert_eq!(
            remove_subagent("Explore:sub-1,Plan:sub-2", "sub-2"),
            Some("Explore:sub-1".into())
        );
    }

    #[test]
    fn remove_subagent_same_type_uses_id_not_position() {
        // Regression: with two Explore subagents running in parallel, stopping
        // the FIRST one (sub-1) must remove that specific entry, not the last
        // occurrence. Old type-based remove_last_subagent got this wrong.
        assert_eq!(
            remove_subagent("Explore:sub-1,Explore:sub-2", "sub-1"),
            Some("Explore:sub-2".into())
        );
    }

    #[test]
    fn remove_subagent_same_type_three_parallel() {
        // Stop the middle one of three same-type parallel subagents.
        assert_eq!(
            remove_subagent("Explore:a,Explore:b,Explore:c", "b"),
            Some("Explore:a,Explore:c".into())
        );
    }

    #[test]
    fn remove_subagent_ignores_id_collision_across_types() {
        // The `:id` match must include the colon prefix so a type name ending
        // with the id substring cannot match by accident.
        assert_eq!(
            remove_subagent("TrailingX:y,Explore:x", "x"),
            Some("TrailingX:y".into())
        );
    }

    #[test]
    fn subagent_lifecycle_two_parallel_same_type_stop_first() {
        // Regression for the parallel-same-type bug. Two Explore subagents
        // start, then the FIRST one (sub-1) completes — id-based removal
        // must leave sub-2 in place.
        let list = append_subagent("", "Explore", "sub-1");
        let list = append_subagent(&list, "Explore", "sub-2");
        assert_eq!(list, "Explore:sub-1,Explore:sub-2");

        let remaining = remove_subagent(&list, "sub-1").unwrap();
        assert_eq!(remaining, "Explore:sub-2");

        let remaining = remove_subagent(&remaining, "sub-2").unwrap();
        assert_eq!(remaining, "");
    }

    #[test]
    fn subagent_lifecycle_mixed_types() {
        let list = append_subagent("", "Explore", "sub-1");
        let list = append_subagent(&list, "Plan", "sub-2");
        assert_eq!(list, "Explore:sub-1,Plan:sub-2");

        // Plan completes, Explore still running
        let remaining = remove_subagent(&list, "sub-2").unwrap();
        assert_eq!(remaining, "Explore:sub-1");
    }

    #[test]
    fn subagent_lifecycle_stop_unknown_id_is_noop() {
        let list = append_subagent("", "Explore", "sub-1");
        assert_eq!(remove_subagent(&list, "sub-999"), None);
    }

    #[test]
    fn should_update_cwd_lifecycle_subagent_start_then_stop() {
        let no_subagents = "";
        let one_subagent = append_subagent(no_subagents, "Explore", "sub-1");

        assert!(should_update_cwd(no_subagents));
        assert!(!should_update_cwd(&one_subagent));

        let after_stop = remove_subagent(&one_subagent, "sub-1").unwrap();
        assert!(should_update_cwd(&after_stop));
    }

    #[test]
    fn should_update_cwd_nested_subagents_require_all_stopped() {
        let list = append_subagent("", "Explore", "sub-1");
        let list = append_subagent(&list, "Plan", "sub-2");
        assert!(!should_update_cwd(&list));

        let list = remove_subagent(&list, "sub-2").unwrap();
        assert!(!should_update_cwd(&list));

        let list = remove_subagent(&list, "sub-1").unwrap();
        assert!(should_update_cwd(&list));
    }

    #[test]
    fn should_update_cwd_race_condition_session_start_before_subagent_start() {
        // Edge case: if subagent's session-start fires BEFORE the parent's
        // subagent-start hook sets @pane_subagents, the cwd would be updated.
        // This documents the known limitation — @pane_subagents is still empty.
        let before_subagent_start_hook = "";
        assert!(
            should_update_cwd(before_subagent_start_hook),
            "known limitation: if session-start races ahead of subagent-start, cwd is updated"
        );
    }
}
