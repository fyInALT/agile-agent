use std::collections::{HashMap, HashSet};

use crate::ext::error::ParseError;

use super::document::{Bundle, Tree};
use super::node::{Node, NodeBehavior, SubTreeNode};

// ── validate_api_version ────────────────────────────────────────────────────

pub fn validate_api_version(tree: &Tree) -> Result<(), ParseError> {
    if tree.api_version.starts_with("decision.agile-agent.io/v") {
        let suffix = &tree.api_version["decision.agile-agent.io/v".len()..];
        if suffix.parse::<u32>().is_ok() {
            return Ok(());
        }
    }
    Err(ParseError::UnsupportedVersion(tree.api_version.clone()))
}

// ── validate_unique_names ───────────────────────────────────────────────────

pub fn validate_unique_names(tree: &Tree) -> Result<(), ParseError> {
    let mut seen = HashSet::new();
    check_unique_names(&tree.spec.root, &mut seen)
}

fn check_unique_names(node: &Node, seen: &mut HashSet<String>) -> Result<(), ParseError> {
    let name = node.name().to_string();
    if !seen.insert(name.clone()) {
        return Err(ParseError::DuplicateName { name });
    }
    for child in node.children() {
        check_unique_names(child, seen)?;
    }
    Ok(())
}

// ── validate_subtree_refs ───────────────────────────────────────────────────

pub fn validate_subtree_refs(tree: &Tree, bundle: &Bundle) -> Result<(), ParseError> {
    check_subtree_refs(&tree.spec.root, bundle)
}

fn check_subtree_refs(node: &Node, bundle: &Bundle) -> Result<(), ParseError> {
    if let Node::SubTree(st) = node {
        if !bundle.subtrees.contains_key(&st.ref_name) {
            return Err(ParseError::UnresolvedSubTree {
                name: st.ref_name.clone(),
            });
        }
    }
    for child in node.children() {
        check_subtree_refs(child, bundle)?;
    }
    Ok(())
}

// ── detect_circular_subtree_refs ────────────────────────────────────────────

pub fn detect_circular_subtree_refs(bundle: &Bundle) -> Result<(), ParseError> {
    for (name, tree) in &bundle.subtrees {
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        if let Some(cycle) = detect_cycle_in_node(&tree.spec.root, bundle, &mut visited, &mut path) {
            return Err(ParseError::CircularSubTreeRef {
                name: cycle.join(" -> "),
            });
        }
    }
    for (name, tree) in &bundle.trees {
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        if let Some(cycle) = detect_cycle_in_node(&tree.spec.root, bundle, &mut visited, &mut path) {
            return Err(ParseError::CircularSubTreeRef {
                name: cycle.join(" -> "),
            });
        }
    }
    Ok(())
}

fn detect_cycle_in_node(
    node: &Node,
    bundle: &Bundle,
    visited: &mut HashSet<String>,
    path: &mut Vec<String>,
) -> Option<Vec<String>> {
    if let Node::SubTree(SubTreeNode { ref_name, .. }) = node {
        if !visited.insert(ref_name.clone()) {
            // Cycle detected
            let start = path.iter().position(|n| n == ref_name).unwrap_or(0);
            let mut cycle = path[start..].to_vec();
            cycle.push(ref_name.clone());
            return Some(cycle);
        }
        path.push(ref_name.clone());
        if let Some(subtree) = bundle.subtrees.get(ref_name) {
            if let Some(cycle) = detect_cycle_in_node(&subtree.spec.root, bundle, visited, path) {
                return Some(cycle);
            }
        }
        path.pop();
        visited.remove(ref_name);
    }
    for child in node.children() {
        if let Some(cycle) = detect_cycle_in_node(child, bundle, visited, path) {
            return Some(cycle);
        }
    }
    None
}

// ── validate_unique_priorities ─────────────────────────────────────────────

pub fn validate_unique_priorities(rules: &[super::document::RuleSpec]) -> Result<(), ParseError> {
    let mut seen = HashSet::new();
    for rule in rules {
        if !seen.insert(rule.priority) {
            return Err(ParseError::DuplicatePriority {
                priority: rule.priority,
            });
        }
    }
    Ok(())
}

// ── validate_bundle ─────────────────────────────────────────────────────────

pub fn validate_bundle(bundle: &Bundle) -> Result<(), ParseError> {
    for (_, tree) in &bundle.trees {
        validate_api_version(tree)?;
        validate_unique_names(tree)?;
        validate_subtree_refs(tree, bundle)?;
    }
    for (_, tree) in &bundle.subtrees {
        validate_api_version(tree)?;
        validate_unique_names(tree)?;
        validate_subtree_refs(tree, bundle)?;
    }
    detect_circular_subtree_refs(bundle)?;
    Ok(())
}
