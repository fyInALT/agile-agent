use std::collections::HashMap;
use std::path::{Path, PathBuf};

use decision_dsl::ast::{
    DslDocument, DslParser, Metadata, OnError, TreeKind, YamlParser,
};
use decision_dsl::ext::command::{AgentCommand, DecisionCommand, HumanCommand};
use decision_dsl::ext::traits::{Fs, FsError};

// ── parse_document: DecisionRules ───────────────────────────────────────────

#[test]
fn parse_decision_rules_basic() {
    let yaml = r#"
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: test-rules
rules:
  - priority: 1
    name: rule1
    then:
      kind: InlineCommand
      payload:
        command:
          kind: Agent
          payload:
            kind: WakeUp
"#;
    let parser = YamlParser::new();
    let doc = parser.parse_document(yaml).unwrap();
    match doc {
        DslDocument::DecisionRules { api_version, metadata, rules } => {
            assert_eq!(api_version, "decision.agile-agent.io/v1");
            assert_eq!(metadata.name, "test-rules");
            assert_eq!(rules.len(), 1);
            assert_eq!(rules[0].name, "rule1");
            assert_eq!(rules[0].priority, 1);
        }
        _ => panic!("expected DecisionRules"),
    }
}

#[test]
fn parse_decision_rules_with_condition() {
    let yaml = r#"
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: rules
rules:
  - priority: 1
    name: check_output
    if:
      kind: OutputContains
      payload:
        pattern: "error"
    then:
      kind: InlineCommand
      payload:
        command:
          kind: Human
          payload:
            kind: EscalateToHuman
            payload:
              reason: "found error"
              context: null
"#;
    let parser = YamlParser::new();
    let doc = parser.parse_document(yaml).unwrap();
    match doc {
        DslDocument::DecisionRules { rules, .. } => {
            assert!(rules[0].condition.is_some());
            assert_eq!(rules[0].name, "check_output");
        }
        _ => panic!("expected DecisionRules"),
    }
}

#[test]
fn parse_decision_rules_with_cooldown_and_reflection() {
    let yaml = r#"
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: rules
rules:
  - priority: 1
    name: guarded
    cooldownMs: 5000
    reflectionMaxRounds: 3
    then:
      kind: InlineCommand
      payload:
        command:
          kind: Agent
          payload:
            kind: WakeUp
"#;
    let parser = YamlParser::new();
    let doc = parser.parse_document(yaml).unwrap();
    match doc {
        DslDocument::DecisionRules { rules, .. } => {
            assert_eq!(rules[0].cooldown_ms, Some(5000));
            assert_eq!(rules[0].reflection_max_rounds, Some(3));
        }
        _ => panic!("expected DecisionRules"),
    }
}

#[test]
fn parse_decision_rules_with_on_error_escalate() {
    let yaml = r#"
apiVersion: decision.agile-agent.io/v1
kind: DecisionRules
metadata:
  name: rules
rules:
  - priority: 1
    name: risky
    on_error: Escalate
    then:
      kind: InlineCommand
      payload:
        command:
          kind: Agent
          payload:
            kind: WakeUp
"#;
    let parser = YamlParser::new();
    let doc = parser.parse_document(yaml).unwrap();
    match doc {
        DslDocument::DecisionRules { rules, .. } => {
            assert_eq!(rules[0].on_error, Some(OnError::Escalate));
        }
        _ => panic!("expected DecisionRules"),
    }
}

// ── parse_document: BehaviorTree ────────────────────────────────────────────

#[test]
fn parse_behavior_tree() {
    let yaml = r#"
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree
metadata:
  name: simple-tree
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
"#;
    let parser = YamlParser::new();
    let doc = parser.parse_document(yaml).unwrap();
    match doc {
        DslDocument::BehaviorTree { api_version, metadata, .. } => {
            assert_eq!(api_version, "decision.agile-agent.io/v1");
            assert_eq!(metadata.name, "simple-tree");
        }
        _ => panic!("expected BehaviorTree"),
    }
}

// ── parse_document: SubTree ─────────────────────────────────────────────────

#[test]
fn parse_subtree() {
    let yaml = r#"
apiVersion: decision.agile-agent.io/v1
kind: SubTree
metadata:
  name: helper
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
"#;
    let parser = YamlParser::new();
    let doc = parser.parse_document(yaml).unwrap();
    match doc {
        DslDocument::SubTree { api_version, metadata, .. } => {
            assert_eq!(api_version, "decision.agile-agent.io/v1");
            assert_eq!(metadata.name, "helper");
        }
        _ => panic!("expected SubTree"),
    }
}

// ── parse_document: error cases ─────────────────────────────────────────────

#[test]
fn parse_invalid_yaml() {
    let parser = YamlParser::new();
    let result = parser.parse_document("not: [ valid yaml");
    assert!(result.is_err());
}

#[test]
fn parse_unknown_kind() {
    let yaml = r#"
apiVersion: decision.agile-agent.io/v1
kind: UnknownThing
metadata:
  name: test
"#;
    let parser = YamlParser::new();
    let result = parser.parse_document(yaml);
    assert!(result.is_err());
}

// ── parse_bundle ────────────────────────────────────────────────────────────

struct MockFs {
    files: HashMap<PathBuf, String>,
}

impl Fs for MockFs {
    fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| FsError::NotFound(path.to_path_buf()))
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FsError> {
        let mut entries = Vec::new();
        for (p, _) in &self.files {
            if let Some(parent) = p.parent() {
                if parent == path {
                    entries.push(p.clone());
                }
            }
        }
        Ok(entries)
    }

    fn modified(&self, _path: &Path) -> Result<std::time::SystemTime, FsError> {
        Ok(std::time::SystemTime::now())
    }
}

#[test]
fn parse_bundle_reads_trees_and_subtrees() {
    let mut files = HashMap::new();
    files.insert(
        PathBuf::from("/bundle/trees/main.yaml"),
        r#"
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree
metadata:
  name: main
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
"#
        .into(),
    );
    files.insert(
        PathBuf::from("/bundle/subtrees/helper.yaml"),
        r#"
apiVersion: decision.agile-agent.io/v1
kind: SubTree
metadata:
  name: helper
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
"#
        .into(),
    );

    let fs = MockFs { files };
    let parser = YamlParser::new();
    let bundle = parser.parse_bundle(Path::new("/bundle"), &fs).unwrap();
    assert_eq!(bundle.trees.len(), 1);
    assert_eq!(bundle.subtrees.len(), 1);
    assert!(bundle.trees.contains_key("main"));
    assert!(bundle.subtrees.contains_key("helper"));
}
