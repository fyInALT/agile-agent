use std::collections::HashSet;

use crate::ext::error::ParseError;

use super::document::{Bundle, Tree};
use super::eval::{Evaluator, EvaluatorRegistry};
use super::node::{Node, NodeBehavior, SubTreeNode};
use super::parser_out::{OutputParser, OutputParserRegistry};

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
    for (_name, tree) in &bundle.subtrees {
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        if let Some(cycle) = detect_cycle_in_node(&tree.spec.root, bundle, &mut visited, &mut path) {
            return Err(ParseError::CircularSubTreeRef {
                name: cycle.join(" -> "),
            });
        }
    }
    for (_name, tree) in &bundle.trees {
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

// ── validate_evaluators ──────────────────────────────────────────────────────

pub fn validate_evaluators(tree: &Tree, registry: &EvaluatorRegistry) -> Result<(), ParseError> {
    check_evaluators_in_node(&tree.spec.root, registry)
}

fn check_evaluators_in_node(node: &Node, registry: &EvaluatorRegistry) -> Result<(), ParseError> {
    match node {
        Node::Condition(cond) => validate_evaluator(&cond.evaluator, registry),
        Node::When(when) => {
            validate_evaluator(&when.condition, registry)?;
            check_evaluators_in_node(&when.action, registry)
        }
        Node::Action(action) => {
            if let Some(eval) = &action.when {
                validate_evaluator(eval, registry)?;
            }
            Ok(())
        }
        _ => {
            for child in node.children() {
                check_evaluators_in_node(child, registry)?;
            }
            Ok(())
        }
    }
}

fn validate_evaluator(evaluator: &Evaluator, registry: &EvaluatorRegistry) -> Result<(), ParseError> {
    match evaluator {
        Evaluator::Custom { name, .. } => {
            // Sprint 3 will implement registry lookup
            // For now, reject all Custom evaluators since registry is empty placeholder
            Err(ParseError::UnknownEvaluatorKind { kind: name.clone() })
        }
        Evaluator::Or { conditions } | Evaluator::And { conditions } => {
            for cond in conditions {
                validate_evaluator(cond, registry)?;
            }
            Ok(())
        }
        Evaluator::Not { condition } => validate_evaluator(condition, registry),
        _ => Ok(()), // Built-in evaluators are always valid
    }
}

// ── validate_parsers ───────────────────────────────────────────────────────────

pub fn validate_parsers(tree: &Tree, registry: &OutputParserRegistry) -> Result<(), ParseError> {
    check_parsers_in_node(&tree.spec.root, registry)
}

fn check_parsers_in_node(node: &Node, registry: &OutputParserRegistry) -> Result<(), ParseError> {
    match node {
        Node::Prompt(prompt) => validate_parser(&prompt.parser, registry),
        _ => {
            for child in node.children() {
                check_parsers_in_node(child, registry)?;
            }
            Ok(())
        }
    }
}

fn validate_parser(parser: &OutputParser, _registry: &OutputParserRegistry) -> Result<(), ParseError> {
    match parser {
        OutputParser::Custom { name, .. } => {
            // Sprint 3 will implement registry lookup
            // For now, reject all Custom parsers since registry is empty placeholder
            Err(ParseError::UnknownParserKind { kind: name.clone() })
        }
        _ => Ok(()), // Built-in parsers are always valid
    }
}

// ── validate_bundle ─────────────────────────────────────────────────────────

pub fn validate_bundle(bundle: &Bundle) -> Result<(), ParseError> {
    let evaluator_registry = EvaluatorRegistry::new();
    let parser_registry = OutputParserRegistry::new();

    for (_, tree) in &bundle.trees {
        validate_api_version(tree)?;
        validate_unique_names(tree)?;
        validate_subtree_refs(tree, bundle)?;
        validate_evaluators(tree, &evaluator_registry)?;
        validate_parsers(tree, &parser_registry)?;
    }
    for (_, tree) in &bundle.subtrees {
        validate_api_version(tree)?;
        validate_unique_names(tree)?;
        validate_subtree_refs(tree, bundle)?;
        validate_evaluators(tree, &evaluator_registry)?;
        validate_parsers(tree, &parser_registry)?;
    }
    detect_circular_subtree_refs(bundle)?;
    Ok(())
}
