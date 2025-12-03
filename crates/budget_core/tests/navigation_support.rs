use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;

pub struct CliOutput {
    pub stdout: String,
    #[allow(dead_code)]
    pub stderr: String,
}

pub struct NavigationTestHarness {
    home: TempDir,
}

impl NavigationTestHarness {
    pub fn new() -> Self {
        Self {
            home: tempfile::tempdir().expect("create temp dir"),
        }
    }

    pub fn home(&self) -> &Path {
        self.home.path()
    }

    #[allow(dead_code)]
    pub fn run_interactive(&self, menu_sequences: &[&str], text_inputs: &[&str]) -> CliOutput {
        self.run_interactive_with_env(menu_sequences, text_inputs, &[])
    }

    #[allow(dead_code)]
    pub fn run_interactive_with_env(
        &self,
        menu_sequences: &[&str],
        text_inputs: &[&str],
        extra_env: &[(&str, &str)],
    ) -> CliOutput {
        assert!(
            !menu_sequences.is_empty(),
            "provide at least one menu sequence"
        );
        let mut cmd = Command::cargo_bin("budget_core_cli").expect("binary exists");
        cmd.env("BUDGET_CORE_HOME", self.home());
        cmd.env("BUFY_TEST_MENU_EVENTS", join_sequences(menu_sequences));
        if !text_inputs.is_empty() {
            cmd.env("BUFY_TEST_TEXT_INPUTS", join_sequences(text_inputs));
        }
        for (key, value) in extra_env {
            cmd.env(key, value);
        }
        let output = cmd.output().expect("run interactive CLI");
        if !output.status.success() {
            panic!(
                "interactive CLI failed: status={}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        CliOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn run_script(&self, script: &str) -> CliOutput {
        let mut cmd = Command::cargo_bin("budget_core_cli").expect("binary exists");
        cmd.env("BUDGET_CORE_HOME", self.home())
            .env("BUDGET_CORE_CLI_SCRIPT", "1")
            .write_stdin(script.to_string());
        let output = cmd.output().expect("run script CLI");
        if !output.status.success() {
            panic!(
                "script CLI failed: status={}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        CliOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
    }
}

#[allow(dead_code)]
fn join_sequences(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("|")
}
