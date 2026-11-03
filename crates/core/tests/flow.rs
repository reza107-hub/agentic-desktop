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
    assert_eq!(adapter.performed(), vec!["click Save"]);
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

#[test]
fn duplicate_elements_give_ambiguous_target() {
    let twin = || AccessibilityNode::new("button").named("OK");
    let adapter = FixtureAdapter::with_tree(
        AccessibilityNode::new("window").with_children(vec![twin(), twin()]),
    );
    let store = temp_store();
    let snap = commands::take_snapshot(&adapter, &store, FIXTURE_APP, false).unwrap();
    let target = ref_of(&snap, "OK");
    let error = commands::act_on_ref(&adapter, &store, &target, Action::Click).unwrap_err();
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
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
