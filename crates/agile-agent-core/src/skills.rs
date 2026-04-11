use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub body: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillRegistry {
    pub discovered: Vec<SkillMetadata>,
    pub enabled_names: BTreeSet<String>,
}

impl SkillRegistry {
    pub fn discover(cwd: &Path) -> Self {
        let roots = default_skill_roots(cwd);
        Self::discover_from_roots(&roots)
    }

    pub fn discover_from_roots(roots: &[PathBuf]) -> Self {
        let mut discovered = Vec::new();
        for root in roots {
            discovered.extend(discover_skills_under(root));
        }

        discovered.sort_by(|a, b| a.name.cmp(&b.name).then(a.path.cmp(&b.path)));
        discovered.dedup_by(|a, b| a.path == b.path);

        Self {
            discovered,
            enabled_names: BTreeSet::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.discovered.is_empty()
    }

    pub fn len(&self) -> usize {
        self.discovered.len()
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled_names.contains(name)
    }

    pub fn toggle(&mut self, name: &str) {
        if self.enabled_names.contains(name) {
            self.enabled_names.remove(name);
        } else {
            self.enabled_names.insert(name.to_string());
        }
    }

    pub fn enabled_count(&self) -> usize {
        self.enabled_names.len()
    }

    pub fn build_injected_prompt(&self, prompt: &str) -> String {
        let enabled_skills: Vec<&SkillMetadata> = self
            .discovered
            .iter()
            .filter(|skill| self.enabled_names.contains(&skill.name))
            .collect();

        if enabled_skills.is_empty() {
            return prompt.to_string();
        }

        let mut context = String::from(
            "[Agile Agent Skill Context]\nThe following local skills are enabled for this turn.\n\n",
        );

        for skill in enabled_skills {
            context.push_str(&format!(
                "## Skill: {}\nPath: {}\n{}\n\n",
                skill.name,
                skill.path.display(),
                skill.body
            ));
        }

        context.push_str("[End Agile Agent Skill Context]\n\n");
        context.push_str(prompt);
        context
    }
}

fn default_skill_roots(cwd: &Path) -> Vec<PathBuf> {
    let mut roots = vec![cwd.join(".agile-agent").join("skills"), cwd.join("skills")];

    if let Some(config_dir) = dirs::config_dir() {
        roots.push(config_dir.join("agile-agent").join("skills"));
    }

    roots
}

fn discover_skills_under(root: &Path) -> Vec<SkillMetadata> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut skills = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let skill_path = path.join("SKILL.md");
        let Ok(body) = fs::read_to_string(&skill_path) else {
            continue;
        };

        if let Some(skill) = parse_skill(&skill_path, &body) {
            skills.push(skill);
        }
    }

    skills
}

fn parse_skill(path: &Path, body: &str) -> Option<SkillMetadata> {
    let fallback_name = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .map(str::to_owned)?;

    let (frontmatter_name, frontmatter_description) = parse_frontmatter(body);
    let description = frontmatter_description
        .or_else(|| first_meaningful_paragraph(body))
        .unwrap_or_else(|| "No description available.".to_string());

    Some(SkillMetadata {
        name: frontmatter_name.unwrap_or(fallback_name),
        description,
        path: path.to_path_buf(),
        body: body.to_string(),
    })
}

fn parse_frontmatter(body: &str) -> (Option<String>, Option<String>) {
    let mut lines = body.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }

    let mut name = None;
    let mut description = None;
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }

        if let Some(value) = trimmed.strip_prefix("name:") {
            name = Some(value.trim().to_string());
        } else if let Some(value) = trimmed.strip_prefix("description:") {
            description = Some(value.trim().trim_matches('"').to_string());
        }
    }

    (name, description)
}

fn first_meaningful_paragraph(body: &str) -> Option<String> {
    let mut paragraph = Vec::new();
    let mut in_frontmatter = false;
    let mut frontmatter_done = false;

    for line in body.lines() {
        let trimmed = line.trim();

        if !frontmatter_done && trimmed == "---" {
            in_frontmatter = !in_frontmatter;
            if !in_frontmatter {
                frontmatter_done = true;
            }
            continue;
        }

        if in_frontmatter || trimmed.is_empty() || trimmed.starts_with('#') {
            if !paragraph.is_empty() {
                break;
            }
            continue;
        }

        paragraph.push(trimmed);
    }

    if paragraph.is_empty() {
        None
    } else {
        Some(paragraph.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::SkillRegistry;
    use std::fs;

    use tempfile::TempDir;

    #[test]
    fn discovers_skill_with_frontmatter() {
        let temp = TempDir::new().expect("tempdir");
        let skill_dir = temp.path().join("skills").join("reviewer");
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: reviewer\ndescription: Reviews code.\n---\n\n# Reviewer\n\nSkill body.",
        )
        .expect("write skill");

        let registry = SkillRegistry::discover_from_roots(&[temp.path().join("skills")]);

        assert_eq!(registry.discovered.len(), 1);
        assert_eq!(registry.discovered[0].name, "reviewer");
        assert_eq!(registry.discovered[0].description, "Reviews code.");
    }

    #[test]
    fn falls_back_to_directory_name_and_body_paragraph() {
        let temp = TempDir::new().expect("tempdir");
        let skill_dir = temp.path().join("skills").join("planner");
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            "# Planner\n\nPlans project work in a structured way.\n\nMore text.",
        )
        .expect("write skill");

        let registry = SkillRegistry::discover_from_roots(&[temp.path().join("skills")]);

        assert_eq!(registry.discovered[0].name, "planner");
        assert_eq!(
            registry.discovered[0].description,
            "Plans project work in a structured way."
        );
    }

    #[test]
    fn toggles_enabled_state() {
        let mut registry = SkillRegistry::default();
        registry.toggle("reviewer");
        assert!(registry.is_enabled("reviewer"));
        registry.toggle("reviewer");
        assert!(!registry.is_enabled("reviewer"));
    }

    #[test]
    fn injected_prompt_contains_enabled_skill_context() {
        let temp = TempDir::new().expect("tempdir");
        let skill_dir = temp.path().join("skills").join("reviewer");
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: reviewer\ndescription: Reviews code.\n---\n\nReview diffs carefully.",
        )
        .expect("write skill");

        let mut registry = SkillRegistry::discover_from_roots(&[temp.path().join("skills")]);
        registry.toggle("reviewer");

        let injected = registry.build_injected_prompt("Hello");

        assert!(injected.contains("[Agile Agent Skill Context]"));
        assert!(injected.contains("## Skill: reviewer"));
        assert!(injected.contains("Review diffs carefully."));
        assert!(injected.ends_with("Hello"));
    }
}
