use decision_dsl::ast::{
    validate_bundle, validate_unique_priorities, ActionNode, Bundle, DslParser, Evaluator,
    Metadata, Node, OutputParser, RuleSpec, Spec, SubTreeNode, ThenSpec, Tree, TreeKind,
};
use decision_dsl::ast::parser::YamlParser;
use decision_dsl::ext::command::{AgentCommand, DecisionCommand};
use decision_dsl::ext::error::ParseError;
use decision_dsl::ext::traits::{Fs, FsError};

fn make_action(name: &str) -> Node {
    Node::Action(ActionNode {
        name: name.into(),
        command: DecisionCommand::Agent(AgentCommand::WakeUp),
        when: None,
    })
}

// ── validate_api_version ────────────────────────────────────────────────────

#[test]
fn valid_api_version() {
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata { name: "t".into(), description: None },
        spec: Spec { root: make_action("a") },
    };
    assert!(decision_dsl::ast::validate_api_version(&tree).is_ok());
}

#[test]
fn invalid_api_version_format() {
    let tree = Tree {
        api_version: "v0".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata { name: "t".into(), description: None },
        spec: Spec { root: make_action("a") },
    };
    let err = decision_dsl::ast::validate_api_version(&tree).unwrap_err();
    assert!(matches!(err, ParseError::UnsupportedVersion(_)));
}

// ── validate_unique_names ───────────────────────────────────────────────────

#[test]
fn unique_names_ok() {
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata { name: "t".into(), description: None },
        spec: Spec {
            root: Node::Selector(decision_dsl::ast::SelectorNode {
                name: "sel".into(),
                children: vec![make_action("a1"), make_action("a2")],
                active_child: None,
            }),
        },
    };
    assert!(decision_dsl::ast::validate_unique_names(&tree).is_ok());
}

#[test]
fn duplicate_names_fails() {
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata { name: "t".into(), description: None },
        spec: Spec {
            root: Node::Selector(decision_dsl::ast::SelectorNode {
                name: "sel".into(),
                children: vec![make_action("dup"), make_action("dup")],
                active_child: None,
            }),
        },
    };
    let err = decision_dsl::ast::validate_unique_names(&tree).unwrap_err();
    assert!(matches!(err, ParseError::DuplicateName { .. }));
}

// ── validate_subtree_refs ───────────────────────────────────────────────────

#[test]
fn subtree_ref_resolves() {
    let mut bundle = Bundle::default();
    bundle.subtrees.insert(
        "helper".into(),
        Tree {
            api_version: "decision.agile-agent.io/v1".into(),
            kind: TreeKind::SubTree,
            metadata: Metadata { name: "helper".into(), description: None },
            spec: Spec { root: make_action("a") },
        },
    );
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata { name: "main".into(), description: None },
        spec: Spec {
            root: Node::SubTree(SubTreeNode {
                name: "sub".into(),
                ref_name: "helper".into(),
                resolved_root: None,
            }),
        },
    };
    assert!(decision_dsl::ast::validate_subtree_refs(&tree, &bundle).is_ok());
}

#[test]
fn unresolved_subtree_ref_fails() {
    let bundle = Bundle::default();
    let tree = Tree {
        api_version: "decision.agile-agent.io/v1".into(),
        kind: TreeKind::BehaviorTree,
        metadata: Metadata { name: "main".into(), description: None },
        spec: Spec {
            root: Node::SubTree(SubTreeNode {
                name: "sub".into(),
                ref_name: "missing".into(),
                resolved_root: None,
            }),
        },
    };
    let err = decision_dsl::ast::validate_subtree_refs(&tree, &bundle).unwrap_err();
    assert!(matches!(err, ParseError::UnresolvedSubTree { .. }));
}

// ── detect_circular_subtree_refs ────────────────────────────────────────────

#[test]
fn no_cycle_ok() {
    let mut bundle = Bundle::default();
    bundle.subtrees.insert(
        "a".into(),
        Tree {
            api_version: "decision.agile-agent.io/v1".into(),
            kind: TreeKind::SubTree,
            metadata: Metadata { name: "a".into(), description: None },
            spec: Spec { root: make_action("a") },
        },
    );
    assert!(decision_dsl::ast::detect_circular_subtree_refs(&bundle).is_ok());
}

#[test]
fn direct_cycle_detected() {
    let mut bundle = Bundle::default();
    bundle.subtrees.insert(
        "a".into(),
        Tree {
            api_version: "decision.agile-agent.io/v1".into(),
            kind: TreeKind::SubTree,
            metadata: Metadata { name: "a".into(), description: None },
            spec: Spec {
                root: Node::SubTree(SubTreeNode {
                    name: "sub".into(),
                    ref_name: "a".into(),
                    resolved_root: None,
                }),
            },
        },
    );
    let err = decision_dsl::ast::detect_circular_subtree_refs(&bundle).unwrap_err();
    assert!(matches!(err, ParseError::CircularSubTreeRef { .. }));
}

// ── validate_unique_priorities ─────────────────────────────────────────────

fn make_rule(priority: u32, name: &str) -> RuleSpec {
    RuleSpec {
        priority,
        name: name.into(),
        condition: None,
        action: ThenSpec::InlineCommand {
            command: DecisionCommand::Agent(AgentCommand::WakeUp),
        },
        cooldown_ms: None,
        reflection_max_rounds: None,
        on_error: None,
    }
}

#[test]
fn unique_priorities_ok() {
    let rules = vec![
        make_rule(1, "a"),
        make_rule(2, "b"),
        make_rule(3, "c"),
    ];
    assert!(validate_unique_priorities(&rules).is_ok());
}

#[test]
fn duplicate_priority_fails() {
    let rules = vec![
        make_rule(1, "a"),
        make_rule(2, "b"),
        make_rule(1, "c"),
    ];
    let err = validate_unique_priorities(&rules).unwrap_err();
    assert!(matches!(err, ParseError::DuplicatePriority { priority: 1 }));
}

// ── validate_bundle integration ─────────────────────────────────────────────

struct MockFs {
    files: std::collections::HashMap<std::path::PathBuf, String>,
}

impl Fs for MockFs {
    fn read_to_string(&self, path: &std::path::Path) -> Result<String, FsError> {
        self.files.get(path).cloned().ok_or_else(|| FsError::NotFound(path.to_path_buf()))
    }
    fn read_dir(&self, path: &std::path::Path) -> Result<Vec<std::path::PathBuf>, FsError> {
        let mut entries = Vec::new();
        for (p, _) in &self.files {
            if let Some(parent) = p.parent() {
                if parent == path { entries.push(p.clone()); }
            }
        }
        Ok(entries)
    }
    fn modified(&self, _path: &std::path::Path) -> Result<std::time::SystemTime, FsError> {
        Ok(std::time::SystemTime::now())
    }
}

#[test]
fn parse_bundle_with_validation_rejects_invalid_api_version() {
    let mut files = std::collections::HashMap::new();
    files.insert(
        std::path::PathBuf::from("/bundle/trees/bad.yaml"),
        r#"
apiVersion: invalid
kind: BehaviorTree
metadata:
  name: bad
spec:
  root:
    kind: Action
    payload:
      name: act
      command:
        kind: Agent
        payload:
          kind: WakeUp
      when: null
"#.into(),
    );

    let fs = MockFs { files };
    let parser = YamlParser::new();
    let result = parser.parse_bundle(std::path::Path::new("/bundle"), &fs);
    match &result { Ok(b) => println!("bundle trees = {}", b.trees.len()), Err(e) => println!("err = {}", e) };
    assert!(result.is_err());
}
