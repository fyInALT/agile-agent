use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ext::error::ParseError;
use crate::ext::traits::Fs;

use super::document::{Bundle, DslDocument, Metadata, Tree, TreeKind};
use super::node::Node;

// ── DslParser trait ─────────────────────────────────────────────────────────

pub trait DslParser {
    fn parse_document(&self, yaml: &str) -> Result<DslDocument, ParseError>;
    fn parse_bundle(&self, dir: &Path, fs: &dyn Fs) -> Result<Bundle, ParseError>;
}

// ── YamlParser ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct YamlParser;

impl YamlParser {
    pub fn new() -> Self {
        Self
    }
}

// Raw YAML representations for parse_document

#[derive(Debug, Serialize, Deserialize)]
struct RawDocument {
    #[serde(rename = "apiVersion")]
    api_version: String,
    kind: String,
    metadata: Metadata,
    #[serde(default)]
    rules: Vec<serde_yaml::Value>,
    spec: Option<RawSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RawSpec {
    root: serde_yaml::Value,
}

impl DslParser for YamlParser {
    fn parse_document(&self, yaml: &str) -> Result<DslDocument, ParseError> {
        let raw: RawDocument = serde_yaml::from_str(yaml)?;

        match raw.kind.as_str() {
            "DecisionRules" => {
                let rules: Vec<super::document::RuleSpec> = raw
                    .rules
                    .into_iter()
                    .map(|v| serde_yaml::from_value(v).map_err(ParseError::from))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(DslDocument::DecisionRules {
                    api_version: raw.api_version,
                    metadata: raw.metadata,
                    rules,
                })
            }
            "BehaviorTree" => {
                let spec = raw.spec.ok_or_else(|| ParseError::MissingProperty("spec"))?;
                let root: Node = serde_yaml::from_value(spec.root)?;
                Ok(DslDocument::BehaviorTree {
                    api_version: raw.api_version,
                    metadata: raw.metadata,
                    root,
                })
            }
            "SubTree" => {
                let spec = raw.spec.ok_or_else(|| ParseError::MissingProperty("spec"))?;
                let root: Node = serde_yaml::from_value(spec.root)?;
                Ok(DslDocument::SubTree {
                    api_version: raw.api_version,
                    metadata: raw.metadata,
                    root,
                })
            }
            other => Err(ParseError::UnknownNodeKind {
                kind: other.into(),
            }),
        }
    }

    fn parse_bundle(&self, dir: &Path, fs: &dyn Fs) -> Result<Bundle, ParseError> {
        let mut bundle = Bundle::default();

        // Read trees/
        let trees_dir = dir.join("trees");
        if let Ok(entries) = fs.read_dir(&trees_dir) {
            for path in entries {
                if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                    let content = fs.read_to_string(&path)?;
                    let doc = self.parse_document(&content)?;
                    if let DslDocument::BehaviorTree { api_version, metadata, root, .. } = doc {
                        let tree = Tree {
                            api_version,
                            kind: TreeKind::BehaviorTree,
                            metadata: metadata.clone(),
                            spec: super::document::Spec { root },
                        };
                        bundle.trees.insert(metadata.name, tree);
                    }
                }
            }
        }

        // Read subtrees/
        let subtrees_dir = dir.join("subtrees");
        if let Ok(entries) = fs.read_dir(&subtrees_dir) {
            for path in entries {
                if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                    let content = fs.read_to_string(&path)?;
                    let doc = self.parse_document(&content)?;
                    if let DslDocument::SubTree { api_version, metadata, root, .. } = doc {
                        let tree = Tree {
                            api_version,
                            kind: TreeKind::SubTree,
                            metadata: metadata.clone(),
                            spec: super::document::Spec { root },
                        };
                        bundle.subtrees.insert(metadata.name, tree);
                    }
                }
            }
        }

        // Read rules.d/ → DecisionRules
        let rules_dir = dir.join("rules.d");
        if let Ok(entries) = fs.read_dir(&rules_dir) {
            let mut all_rules = Vec::new();
            let mut metadata = None;
            let mut api_version = None;
            for path in entries {
                if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                    let content = fs.read_to_string(&path)?;
                    let doc = self.parse_document(&content)?;
                    if let DslDocument::DecisionRules {
                        api_version: av,
                        metadata: md,
                        rules,
                    } = doc
                    {
                        api_version = Some(av);
                        metadata = Some(md);
                        all_rules.extend(rules);
                    }
                }
            }
            if let (Some(av), Some(md)) = (api_version, metadata) {
                let root = super::node::Node::Selector(super::node::SelectorNode {
                    name: format!("{}_root", md.name),
                    children: vec![],
                    active_child: None,
                });
                let tree = Tree {
                    api_version: av,
                    kind: TreeKind::BehaviorTree,
                    metadata: md.clone(),
                    spec: super::document::Spec { root },
                };
                bundle.trees.insert(md.name, tree);
            }
        }

        super::validate::validate_bundle(&bundle)?;
        Ok(bundle)
    }
}
