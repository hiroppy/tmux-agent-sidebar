//! Security regression tests for the shell commands installed by `agent-sidebar.conf`.
//!
//! These tests use the real configuration and a private tmux socket. The probe
//! executable only records its argv, while deliberately named helper executables
//! record any shell syntax that tmux accidentally evaluates.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

const CONFIG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/agent-sidebar.conf");
const SESSION: &str = "agent-sidebar-security";

struct TmuxServer {
    socket: PathBuf,
}

impl TmuxServer {
    fn start(socket: PathBuf, cwd: &Path) -> Self {
        let server = Self { socket };
        server.run([
            OsStr::new("-f"),
            OsStr::new("/dev/null"),
            OsStr::new("new-session"),
            OsStr::new("-d"),
            OsStr::new("-s"),
            OsStr::new(SESSION),
            OsStr::new("-x"),
            OsStr::new("80"),
            OsStr::new("-y"),
            OsStr::new("24"),
            OsStr::new("-c"),
            cwd.as_os_str(),
            OsStr::new("sleep 30"),
        ]);
        server
    }

    fn command(&self) -> Command {
        let mut command = Command::new("tmux");
        command.arg("-S").arg(&self.socket);
        command.env_remove("TMUX");
        command.env_remove("TMUX_PANE");
        command.env("LC_ALL", "C");
        command
    }

    fn output<I, S>(&self, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command()
            .args(args)
            .output()
            .expect("failed to execute tmux; these integration tests require tmux")
    }

    fn run<I, S>(&self, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.output(args);
        assert!(
            output.status.success(),
            "tmux command failed (status {}):\nstdout: {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn stdout<I, S>(&self, args: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        String::from_utf8(self.run(args).stdout).expect("tmux output was not UTF-8")
    }

    fn set_option(&self, name: &str, value: &OsStr) {
        self.run([
            OsStr::new("set-option"),
            OsStr::new("-g"),
            OsStr::new(name),
            value,
        ]);
    }

    fn set_environment(&self, name: &str, value: &OsStr) {
        self.run([
            OsStr::new("set-environment"),
            OsStr::new("-g"),
            OsStr::new(name),
            value,
        ]);
    }

    fn source_config(&self) -> Output {
        self.output([OsStr::new("source-file"), OsStr::new(CONFIG)])
    }

    fn new_window(&self, cwd: &Path) -> Output {
        self.output([
            OsStr::new("new-window"),
            OsStr::new("-d"),
            OsStr::new("-t"),
            OsStr::new(SESSION),
            OsStr::new("-c"),
            cwd.as_os_str(),
            OsStr::new("sleep 30"),
        ])
    }
}

impl Drop for TmuxServer {
    fn drop(&mut self) {
        let _ = self.command().arg("kill-server").output();
    }
}

struct Fixture {
    server: TmuxServer,
    _temp: TempDir,
    root: PathBuf,
    bin_dir: PathBuf,
    probe: PathBuf,
    probe_output: PathBuf,
    injection_marker: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("failed to create temporary test directory");
        let root =
            fs::canonicalize(temp.path()).expect("failed to canonicalize temporary directory");
        let bin_dir = root.join("bin");
        fs::create_dir(&bin_dir).expect("failed to create fake bin directory");

        let probe = bin_dir.join("sidebar-probe");
        write_executable(
            &probe,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$PROBE_OUTPUT\"\n",
        );

        for name in [
            "sidebar-injected-cwd-dollar",
            "sidebar-injected-cwd-backtick",
            "sidebar-injected-key",
            "sidebar-injected-key-all",
            "sidebar-injected-binary",
            "sidebar-injected-delay",
        ] {
            write_executable(
                &bin_dir.join(name),
                "#!/bin/sh\nprintf 'executed: %s\\n' \"$0\" >> \"$INJECTION_MARKER\"\n",
            );
        }

        let probe_output = root.join("probe.argv");
        let injection_marker = root.join("injection-ran");
        let server = TmuxServer::start(root.join("tmux.sock"), &root);

        let old_path = std::env::var_os("PATH").unwrap_or_default();
        let mut path = bin_dir.as_os_str().to_os_string();
        path.push(":");
        path.push(old_path);
        server.set_environment("PATH", &path);
        server.set_environment("PROBE_OUTPUT", probe_output.as_os_str());
        server.set_environment("INJECTION_MARKER", injection_marker.as_os_str());

        Self {
            server,
            _temp: temp,
            root,
            bin_dir,
            probe,
            probe_output,
            injection_marker,
        }
    }

    fn configure_probe(&self) {
        self.server
            .set_option("@agent_sidebar_bin", self.probe.as_os_str());
        // Set behavioral inputs explicitly: these tests are not assertions about
        // the plugin's defaults.
        self.server
            .set_option("@sidebar_auto_create", OsStr::new("on"));
        self.server
            .set_option("@sidebar_auto_create_delay", OsStr::new("0"));
        self.server.set_option("@sidebar_key", OsStr::new("e"));
        self.server.set_option("@sidebar_key_all", OsStr::new("E"));
    }

    fn assert_config_loaded(&self, output: &Output) {
        assert!(
            output.status.success(),
            "agent-sidebar.conf failed to load:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn wait_for_probe(&self) {
        wait_for_path(&self.probe_output);
    }

    fn probe_args(&self) -> Vec<String> {
        fs::read_to_string(&self.probe_output)
            .expect("probe did not record argv")
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn assert_no_injection(&self) {
        assert!(
            !self.injection_marker.exists(),
            "shell syntax was executed:\n{}",
            fs::read_to_string(&self.injection_marker).unwrap_or_default()
        );
    }
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
    let mut permissions = fs::metadata(path)
        .unwrap_or_else(|error| panic!("failed to stat {}: {error}", path.display()))
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions)
        .unwrap_or_else(|error| panic!("failed to make {} executable: {error}", path.display()));
}

fn wait_for_path(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        // This is a condition wait for an external tmux process, not a delay
        // used to make an ordering race pass.
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn automatic_new_window_passes_a_shell_hostile_cwd_as_one_literal_argument() {
    let fixture = Fixture::new();
    fixture.configure_probe();
    fixture.assert_config_loaded(&fixture.server.source_config());

    let hostile_cwd = fixture.root.join(
        "cwd $(sidebar-injected-cwd-dollar) `sidebar-injected-cwd-backtick` \"double\" 'single'; semi colon",
    );
    fs::create_dir(&hostile_cwd).expect("failed to create hostile cwd");

    let new_window = fixture.server.new_window(&hostile_cwd);
    assert!(
        new_window.status.success(),
        "new-window failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&new_window.stdout),
        String::from_utf8_lossy(&new_window.stderr)
    );
    fixture.wait_for_probe();

    let args = fixture.probe_args();
    assert_eq!(args.len(), 4, "probe argv was {args:?}");
    assert_eq!(args[0], "toggle");
    assert_eq!(args[1], "--create-only");
    assert!(
        args[2].starts_with('@'),
        "expected a tmux window id, got {args:?}"
    );
    assert_eq!(args[3], hostile_cwd.to_string_lossy());
    fixture.assert_no_injection();
}

#[test]
fn installed_run_shell_commands_retain_shell_escaped_dynamic_formats() {
    let fixture = Fixture::new();
    fixture.configure_probe();
    fixture.assert_config_loaded(&fixture.server.source_config());

    let new_window_hook = fixture
        .server
        .stdout(["show-hooks", "-g", "after-new-window"]);
    for format in [
        "#{q:@agent_sidebar_bin}",
        "#{q:window_id}",
        "#{q:pane_current_path}",
    ] {
        assert!(
            new_window_hook.contains(format),
            "after-new-window must retain {format} until hook execution; got:\n{new_window_hook}"
        );
    }

    let toggle_binding = fixture.server.stdout(["list-keys", "-T", "prefix", "e"]);
    for delayed_format in ["#{q:window_id}", "#{q:pane_current_path}"] {
        assert!(
            toggle_binding.contains(delayed_format),
            "toggle binding must retain delayed {delayed_format} until key execution; got:\n{toggle_binding}"
        );
    }

    let pane_exited_hook = fixture.server.stdout(["show-hooks", "-g", "pane-exited"]);
    for format in ["#{q:@agent_sidebar_bin}", "#{q:window_id}"] {
        assert!(
            pane_exited_hook.contains(format),
            "pane-exited must retain {format} until hook execution; got:\n{pane_exited_hook}"
        );
    }
}

#[test]
fn configured_keys_and_binary_cannot_execute_shell_syntax_while_loading() {
    for (option, helper) in [
        ("@sidebar_key", "sidebar-injected-key"),
        ("@sidebar_key_all", "sidebar-injected-key-all"),
        ("@agent_sidebar_bin", "sidebar-injected-binary"),
    ] {
        let fixture = Fixture::new();
        fixture.configure_probe();
        let payload = format!("\"; {helper}; printf \"");
        fixture.server.set_option(option, OsStr::new(&payload));

        // An invalid key is allowed to make tmux reject the binding. The
        // security contract is that parsing it never executes the payload.
        let _ = fixture.server.source_config();
        fixture.assert_no_injection();
    }
}

#[test]
fn configured_delay_cannot_inject_a_command_before_the_probe() {
    let fixture = Fixture::new();
    fixture.configure_probe();
    fixture.server.set_option(
        "@sidebar_auto_create_delay",
        OsStr::new("0\"; sidebar-injected-delay; : \""),
    );
    fixture.assert_config_loaded(&fixture.server.source_config());

    let cwd = fixture.root.join("delay-target");
    fs::create_dir(&cwd).expect("failed to create delayed hook cwd");
    let _ = fixture.server.new_window(&cwd);
    fixture.wait_for_probe();

    assert_eq!(
        fixture.probe_args(),
        vec![
            "toggle".to_owned(),
            "--create-only".to_owned(),
            "@1".to_owned(),
            cwd.to_string_lossy().into_owned(),
        ]
    );
    fixture.assert_no_injection();
}
