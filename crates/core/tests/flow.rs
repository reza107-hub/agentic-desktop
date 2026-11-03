use agentic_desktop_core::fixture::{FIXTURE_APP, FixtureAdapter};
use agentic_desktop_core::node::AccessibilityNode;
use agentic_desktop_core::refs::parse_ref;
use agentic_desktop_core::{Action, ErrorCode, SnapshotStore, commands, snapshot};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static N: AtomicU32 = AtomicU32::new(0);

fn temp_store() -> SnapshotStore {
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir: PathBuf =
        std::env::temp_dir().join(format!("agentic-desktop-test-{}-{n}", std::process::id()));
    SnapshotStore::new(dir)
}

fn ref_of(snapshot: &serde_json::Value, name: &str) -> String {
    fn find(node: &serde_json::Value, name: &str) -> Option<String> {
        if node["name"] == name {
            return node["ref_id"].as_str().map(str::to_string);
        }
        node["children"]
            .as_array()?
            .iter()
            .find_map(|c| find(c, name))
    }
    find(&snapshot["tree"], name).expect("ref for name")
}

#[test]
fn refs_are_depth_first_and_snapshot_qualified() {
    let result = snapshot::build(&FixtureAdapter::new(), FIXTURE_APP, false).unwrap();
    assert_eq!(result.refmap.len(), 3);
    let save = &result.tree.children[0].children[0];
    assert_eq!(
        save.ref_id.as_deref(),
        Some(format!("@{}:e1", result.snapshot_id).as_str())
    );
    assert!(
        result.tree.children[2].ref_id.is_none(),
        "static text gets no ref"
    );
}

#[test]
fn bare_and_malformed_refs_are_rejected() {
    for bad in ["@e8", "e8", "@s1:8", "@:e1", "@s1:e", "@s/../x:e1"] {
        assert_eq!(
            parse_ref(bad).unwrap_err().code,
            ErrorCode::InvalidArgs,
            "{bad}"
        );
    }
    assert!(parse_ref("@s8f3k2p9:e8").is_ok());
}

#[test]
fn interactive_only_prunes_unreferenced_branches() {
    let result = snapshot::build(&FixtureAdapter::new(), FIXTURE_APP, true).unwrap();
    assert_eq!(result.tree.children.len(), 2);
    assert!(
        result
            .tree
            .children
            .iter()
            .all(|c| c.ref_id.is_some() || !c.children.is_empty())
    );
}

#[test]
fn click_by_ref_reaches_the_adapter() {
    let adapter = FixtureAdapter::new();
    let store = temp_store();
    let snap = commands::take_snapshot(&adapter, &store, FIXTURE_APP, false).unwrap();
    let target = ref_of(&snap, "Save");
    commands::act_on_ref(&adapter, &store, &target, Action::Click).unwrap();
    assert_eq!(adapter.performed(), vec!["click Save [0, 0]"]);
}

#[test]
fn removed_element_gives_stale_ref() {
    let adapter = FixtureAdapter::new();
    let store = temp_store();
    let snap = commands::take_snapshot(&adapter, &store, FIXTURE_APP, false).unwrap();
    let target = ref_of(&snap, "Save");
    adapter.set_tree(AccessibilityNode::new("window").named("Demo Window"));
    let error = commands::act_on_ref(&adapter, &store, &target, Action::Click).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert!(adapter.performed().is_empty());
}

fn ok_button() -> AccessibilityNode {
    AccessibilityNode::new("button").named("OK")
}

fn row(label: &str) -> AccessibilityNode {
    AccessibilityNode::new("group")
        .named(label)
        .with_children(vec![ok_button()])
}

fn first_ref(snapshot: &serde_json::Value, wanted: usize) -> String {
    let mut refs = Vec::new();
    fn collect(node: &serde_json::Value, out: &mut Vec<String>) {
        if let Some(id) = node["ref_id"].as_str() {
            out.push(id.to_string());
        }
        for child in node["children"].as_array().into_iter().flatten() {
            collect(child, out);
        }
    }
    collect(&snapshot["tree"], &mut refs);
    refs.remove(wanted)
}

#[test]
fn looks_alike_siblings_are_told_apart_by_position() {
    let adapter = FixtureAdapter::with_tree(
        AccessibilityNode::new("window").with_children(vec![ok_button(), ok_button()]),
    );
    let store = temp_store();
    let snap = commands::take_snapshot(&adapter, &store, FIXTURE_APP, false).unwrap();
    let second = first_ref(&snap, 1);
    let report = commands::act_on_ref(&adapter, &store, &second, Action::Click).unwrap();
    assert_eq!(report["matched_by"], "position");
    assert_eq!(adapter.performed(), vec!["click OK [1]"]);
}

#[test]
fn identical_buttons_in_different_rows_resolve_to_their_own_row() {
    let adapter = FixtureAdapter::with_tree(
        AccessibilityNode::new("window").with_children(vec![row("A"), row("B")]),
    );
    let store = temp_store();
    let snap = commands::take_snapshot(&adapter, &store, FIXTURE_APP, false).unwrap();
    commands::act_on_ref(&adapter, &store, &first_ref(&snap, 1), Action::Click).unwrap();
    assert_eq!(adapter.performed(), vec!["click OK [1, 0]"]);
}

#[test]
fn positional_ref_goes_stale_when_its_position_disappears() {
    let adapter = FixtureAdapter::with_tree(
        AccessibilityNode::new("window").with_children(vec![ok_button(), ok_button()]),
    );
    let store = temp_store();
    let snap = commands::take_snapshot(&adapter, &store, FIXTURE_APP, false).unwrap();
    let second = first_ref(&snap, 1);
    adapter.set_tree(AccessibilityNode::new("window").with_children(vec![ok_button()]));
    let error = commands::act_on_ref(&adapter, &store, &second, Action::Click).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn unique_ref_becomes_ambiguous_when_a_twin_appears() {
    let adapter = FixtureAdapter::with_tree(
        AccessibilityNode::new("window").with_children(vec![ok_button()]),
    );
    let store = temp_store();
    let snap = commands::take_snapshot(&adapter, &store, FIXTURE_APP, false).unwrap();
    let only = first_ref(&snap, 0);
    adapter
        .set_tree(AccessibilityNode::new("window").with_children(vec![ok_button(), ok_button()]));
    let error = commands::act_on_ref(&adapter, &store, &only, Action::Click).unwrap_err();
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
}

#[test]
fn truncated_trees_report_complete_false() {
    let mut cut = AccessibilityNode::new("window").with_children(vec![ok_button()]);
    cut.subtree_truncated = true;
    let adapter = FixtureAdapter::with_tree(cut);
    let store = temp_store();
    let snap = commands::take_snapshot(&adapter, &store, FIXTURE_APP, false).unwrap();
    assert_eq!(snap["complete"], false);
    assert_eq!(snap["truncated"], true);
    let full = commands::take_snapshot(&FixtureAdapter::new(), &store, FIXTURE_APP, false).unwrap();
    assert_eq!(full["complete"], true);
}

#[test]
fn unknown_snapshot_and_app_report_clear_codes() {
    let adapter = FixtureAdapter::new();
    let store = temp_store();
    let error = commands::act_on_ref(&adapter, &store, "@sdoesnot:e1", Action::Click).unwrap_err();
    assert_eq!(error.code, ErrorCode::SnapshotNotFound);
    let error = commands::take_snapshot(&adapter, &store, "Nope", false).unwrap_err();
    assert_eq!(error.code, ErrorCode::AppNotFound);
}

#[test]
fn generic_text_actions_do_not_earn_a_ref() {
    let label = AccessibilityNode::new("statictext")
        .named("7")
        .with_actions(&[
            "clipboard.copy",
            "selection.select-all",
            "menu.popup",
            "link.copy",
        ]);
    let real = AccessibilityNode::new("statictext")
        .named("Open")
        .with_actions(&["clipboard.copy", "default.activate"]);
    let adapter = FixtureAdapter::with_tree(
        AccessibilityNode::new("window").with_children(vec![label, real]),
    );
    let result = snapshot::build(&adapter, FIXTURE_APP, false).unwrap();
    assert_eq!(result.refmap.len(), 1);
    assert!(result.tree.children[0].ref_id.is_none());
    assert!(result.tree.children[1].ref_id.is_some());
}

#[test]
fn clipboard_round_trips_and_clears() {
    let adapter = FixtureAdapter::new();
    assert_eq!(commands::clipboard_get(&adapter).unwrap()["text"], "");
    let set = commands::clipboard_set(&adapter, "héllo").unwrap();
    assert_eq!(set["chars"], 5);
    let got = commands::clipboard_get(&adapter).unwrap();
    assert_eq!(got["text"], "héllo");
    commands::clipboard_clear(&adapter).unwrap();
    assert_eq!(commands::clipboard_get(&adapter).unwrap()["text"], "");
}

#[test]
fn oversized_clipboard_text_is_refused() {
    let adapter = FixtureAdapter::new();
    let big = "x".repeat(commands::MAX_CLIPBOARD_BYTES + 1);
    let error = commands::clipboard_set(&adapter, &big).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgs);
}

#[test]
fn backends_without_a_clipboard_say_so() {
    struct Bare;
    impl agentic_desktop_core::PlatformAdapter for Bare {
        fn platform(&self) -> &'static str {
            "bare"
        }
    }
    let error = commands::clipboard_get(&Bare).unwrap_err();
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    let error = commands::clipboard_clear(&Bare).unwrap_err();
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
}

#[test]
fn press_and_mouse_commands_reach_the_adapter() {
    let adapter = FixtureAdapter::new();
    commands::press(&adapter, "Ctrl+Shift+T").unwrap();
    commands::mouse_move(&adapter, 10.0, 20.0).unwrap();
    commands::mouse_click(&adapter, 5.0, 6.0, "right", 2).unwrap();
    commands::mouse_wheel(&adapter, 0, -3).unwrap();
    let log = adapter.input_log();
    assert_eq!(log.len(), 4);
    assert!(log[0].contains("Ctrl") && log[0].contains("Shift"));
    assert_eq!(log[2], "click 5 6 Right 2");
    assert_eq!(log[3], "wheel 0 -3");
}

#[test]
fn bad_input_is_refused_before_reaching_the_adapter() {
    let adapter = FixtureAdapter::new();
    assert_eq!(
        commands::press(&adapter, "ctrl+banana").unwrap_err().code,
        ErrorCode::InvalidArgs
    );
    assert_eq!(
        commands::mouse_move(&adapter, -1.0, 0.0).unwrap_err().code,
        ErrorCode::InvalidArgs
    );
    assert_eq!(
        commands::mouse_move(&adapter, f64::NAN, 0.0)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgs
    );
    assert_eq!(
        commands::mouse_click(&adapter, 1.0, 1.0, "left", 4)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgs
    );
    assert_eq!(
        commands::mouse_click(&adapter, 1.0, 1.0, "nope", 1)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgs
    );
    assert_eq!(
        commands::mouse_wheel(&adapter, 0, 0).unwrap_err().code,
        ErrorCode::InvalidArgs
    );
    assert_eq!(
        commands::mouse_wheel(&adapter, 0, 5000).unwrap_err().code,
        ErrorCode::InvalidArgs
    );
    assert!(adapter.input_log().is_empty());
}

#[test]
fn screenshot_writes_a_png_and_never_overwrites() {
    let adapter = FixtureAdapter::new();
    let store = temp_store();
    let first = commands::screenshot(&adapter, &store, None).unwrap();
    assert_eq!(first["width"], 4);
    let path = first["path"].as_str().unwrap().to_string();
    assert!(std::path::Path::new(&path).is_file());
    let error = commands::screenshot(&adapter, &store, Some(&path)).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgs);
}

#[test]
fn backends_without_input_or_screenshots_say_so() {
    struct Bare;
    impl agentic_desktop_core::PlatformAdapter for Bare {
        fn platform(&self) -> &'static str {
            "bare"
        }
    }
    assert_eq!(
        commands::press(&Bare, "a").unwrap_err().code,
        ErrorCode::PlatformNotSupported
    );
    assert_eq!(
        commands::mouse_wheel(&Bare, 1, 0).unwrap_err().code,
        ErrorCode::PlatformNotSupported
    );
    let store = temp_store();
    assert_eq!(
        commands::screenshot(&Bare, &store, None).unwrap_err().code,
        ErrorCode::PlatformNotSupported
    );
}
