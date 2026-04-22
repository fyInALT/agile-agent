use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use agent_core::backlog::BacklogState;
use agent_core::backlog::TodoItem;
use agent_core::backlog::TodoStatus;
use agent_core::workplace_store::WORKPLACES_ROOT_ENV;
use agent_core::workplace_store::WorkplaceStore;
use tempfile::TempDir;

pub struct RuntimeHarness {
    _home: TempDir,
    _data: TempDir,
    pub workdir: TempDir,
    pub provider_log: PathBuf,
    pub fake_claude_path: PathBuf,
    pub fake_codex_path: PathBuf,
    workplace: WorkplaceStore,
    workplaces_root: PathBuf,
}

impl Default for RuntimeHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeHarness {
    pub fn new() -> Self {
        let home = TempDir::new().expect("temp home");
        let data = TempDir::new().expect("temp data");
        let workdir = TempDir::new().expect("temp workdir");
        let workplace_root = home.path().join(".agile-agent").join("workplaces");
        let workplace =
            WorkplaceStore::for_root(workdir.path(), workplace_root.clone()).expect("workplace");
        workplace.ensure().expect("ensure workplace");
        let bin_dir = home.path().join("bin");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        let provider_log = home.path().join("provider.log");
        let fake_claude_path = bin_dir.join("fake-claude");
        let fake_codex_path = bin_dir.join("fake-codex");
        write_fake_claude(&fake_claude_path, &provider_log);
        write_fake_codex(&fake_codex_path, &provider_log);

        Self {
            _home: home,
            _data: data,
            workdir,
            provider_log,
            fake_claude_path,
            fake_codex_path,
            workplace,
            workplaces_root: workplace_root,
        }
    }

    pub fn workplace(&self) -> &WorkplaceStore {
        &self.workplace
    }

    pub fn write_backlog_with_ready_todo(&self, title: &str) {
        let mut backlog = BacklogState::default();
        backlog.push_todo(TodoItem {
            id: "todo-1".to_string(),
            title: title.to_string(),
            description: title.to_string(),
            priority: 1,
            status: TodoStatus::Ready,
            acceptance_criteria: Vec::new(),
            dependencies: Vec::new(),
            source: "test".to_string(),
        });
        agent_core::backlog_store::save_backlog_for_workplace(&backlog, &self.workplace)
            .expect("save backlog");
    }

    pub fn overwrite_backlog_with_ready_todo(&self, title: &str) {
        self.write_backlog_with_ready_todo(title);
    }

    pub fn run_cli(&self, args: &[&str]) -> std::process::Output {
        self.run_with_env(args, &self.fake_claude_path, "definitely-not-real-codex")
    }

    pub fn run_cli_with_codex(&self, args: &[&str]) -> std::process::Output {
        self.run_with_env(args, "definitely-not-real-claude", &self.fake_codex_path)
    }

    pub fn agent_dir(&self) -> PathBuf {
        self.workplace.agents_dir().join("agent_001")
    }

    pub fn read_provider_log(&self) -> String {
        fs::read_to_string(&self.provider_log).unwrap_or_default()
    }

    fn run_with_env(
        &self,
        args: &[&str],
        claude_path: impl AsRef<std::ffi::OsStr>,
        codex_path: impl AsRef<std::ffi::OsStr>,
    ) -> std::process::Output {
        let bin = env::var("CARGO_BIN_EXE_agile-agent")
            .expect("CARGO_BIN_EXE_agile-agent must be set by cargo test");
        let mut command = Command::new(bin);
        command.current_dir(self.workdir.path());
        command.args(args);
        command.env("HOME", self._home.path());
        command.env("XDG_DATA_HOME", self._data.path());
        command.env("AGILE_AGENT_CLAUDE_PATH", claude_path);
        command.env("AGILE_AGENT_CODEX_PATH", codex_path);
        // Set workplaces root to prevent using ~/.agile-agent/workplaces
        command.env(WORKPLACES_ROOT_ENV, &self.workplaces_root);
        command.output().expect("run agile-agent binary")
    }
}

fn write_fake_claude(script_path: &Path, provider_log: &Path) {
    // Build script with simple string concatenation to avoid format string escaping issues
    let script = String::new()
        + "#!/usr/bin/env bash\n"
        + "set -euo pipefail\n"
        + "if [[ \"${1-}\" == \"--version\" ]]; then\n"
        + "  echo \"fake-claude 0.1.0\"\n"
        + "  exit 0\n"
        + "fi\n"
        + "resume=\"<none>\"\n"
        + "for ((i=1; i<=$#; i++)); do\n"
        + "  if [[ \"${!i}\" == \"--resume\" ]]; then\n"
        + "    j=$((i+1))\n"
        + "    resume=\"${!j}\"\n"
        + "  fi\n"
        + "done\n"
        + &format!(
            "echo \"resume=$resume\" >> \"{}\"\n",
            provider_log.display()
        )
        + "cat >/dev/null\n"
        + "session=\"sess-cli-1\"\n"
        + "if [[ \"$resume\" != \"<none>\" ]]; then\n"
        + "  session=\"$resume\"\n"
        + "fi\n"
        + "printf '{\"type\":\"system\",\"session_id\":\"%s\"}\\n' \"$session\"\n"
        + "printf '{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"done\"}]}}}\\n'\n"
        + "printf '{\"type\":\"result\",\"session_id\":\"%s\",\"is_error\":false}\\n' \"$session\"\n";

    fs::write(script_path, script).expect("write fake claude");
    let mut permissions = fs::metadata(script_path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(script_path, permissions).expect("chmod");
}

fn write_fake_codex(script_path: &Path, provider_log: &Path) {
    // Build script with simple string concatenation to avoid format string escaping issues
    let log_path = provider_log.display().to_string();
    let script = String::new()
        + "#!/usr/bin/env bash\n"
        + "set -euo pipefail\n"
        + "if [[ \"${1-}\" == \"--version\" ]]; then\n"
        + "  echo \"codex-cli 0.1.0\"\n"
        + "  exit 0\n"
        + "fi\n"
        + "\n"
        + "# Default thread ID\n"
        + "thread_id=\"thr-cli-1\"\n"
        + "\n"
        + "# Parse args to detect exec mode and resume\n"
        + "args=(\"$@\")\n"
        + "for ((i=0; i<${#args[@]}; i++)); do\n"
        + "  if [[ \"${args[i]}\" == \"resume\" ]]; then\n"
        + "    # Next arg should be the thread_id\n"
        + "    if ((i+1 < ${#args[@]})); then\n"
        + "      thread_id=\"${args[i+1]}\"\n"
        + &format!("      echo \"resume=$thread_id\" >> \"{}\"\n", log_path)
        + "    fi\n"
        + "    break\n"
        + "  fi\n"
        + "done\n"
        + "\n"
        + "# Output JSONL format for codex exec --json\n"
        + "printf '{\"type\":\"thread.started\",\"thread_id\":\"%s\"}\\n' \"$thread_id\"\n"
        + "printf '{\"type\":\"turn.started\"}\\n'\n"
        + "printf '{\"type\":\"item.completed\",\"item\":{\"id\":\"item-1\",\"type\":\"agent_message\",\"text\":\"done\"}}\\n'\n"
        + "printf '{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":10,\"output_tokens\":10}}\\n'\n";

    fs::write(script_path, script).expect("write fake codex");
    let mut permissions = fs::metadata(script_path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(script_path, permissions).expect("chmod");
}
