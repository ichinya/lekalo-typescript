use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn lekalo_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lekalo"))
        .args(args)
        .current_dir(alias_free_path(dir))
        .env_remove("LEKALO_PROJECT")
        .output()
        .expect("run the real lekalo binary")
}

/// The selection policy denies alias-spelled working directories
/// (`structure.selection-alias`) before any command logic runs; chdir
/// the child into the resolved spelling, stripped of the `\\?\` verbatim
/// prefix `canonicalize` produces on Windows.
fn alias_free_path(path: &Path) -> PathBuf {
    let canonical = path.canonicalize().expect("fixture path must exist");
    #[cfg(windows)]
    match canonical.to_string_lossy().strip_prefix(r"\\?\") {
        // `\\?\C:\...` -> `C:\...`; UNC (`\\?\UNC\...`) stays verbatim.
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => canonical,
    }
    #[cfg(not(windows))]
    canonical
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout utf8")
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr utf8")
}

fn exit_code(output: &Output) -> u8 {
    output.status.code().expect("exit code") as u8
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_str(stdout_text(output).trim()).expect("stdout json envelope")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join(name)
}

/// Copy a fixture into a fresh alias-free temp directory; adopt runs on
/// the copy so the committed fixtures stay pristine evidence.
fn materialize(name: &str) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("repo");
    copy_tree(&fixture(name), &root);
    (temp, root)
}

/// Recursively copy `from` into `to` (regular files and directories only;
/// the fixtures carry no links).
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create copy root");
    for entry in fs::read_dir(from).expect("read fixture") {
        let entry = entry.expect("fixture entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("entry type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy fixture file");
        }
    }
}

/// The sorted `(relative path, bytes)` of every regular file below `root`,
/// skipping the canonical `lekalo/` tree when `skip_lekalo` is set.
fn snapshot(root: &Path, skip_lekalo: bool) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    walk(root, root, skip_lekalo, &mut out);
    out.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
    out
}

fn walk(root: &Path, dir: &Path, skip_lekalo: bool, out: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(dir).expect("walk directory") {
        let entry = entry.expect("walk entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        if skip_lekalo && (name == "lekalo" || name == ".lekalo") {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .expect("walk below root")
            .to_string_lossy()
            .replace('\\', "/");
        if entry.file_type().expect("walk type").is_dir() {
            walk(root, &entry.path(), skip_lekalo, out);
        } else {
            out.push((relative, fs::read(entry.path()).expect("walk read")));
        }
    }
}

/// The Node.js monorepo dry-run plans every write, detects the workspace
/// with provenance, and changes nothing on disk.
#[test]
fn adopt_dry_run_plans_every_write_without_touching_the_tree() {
    let (_temp, root) = materialize("tests/fixtures/adopt/node-monorepo");
    let before = snapshot(&root, false);
    let output = lekalo_in(
        &root,
        &[
            "--json",
            "init",
            "--adopt",
            "--target",
            "node-typescript",
            "--dry-run",
        ],
    );
    assert_eq!(exit_code(&output), 0, "{}", stderr_text(&output));
    let envelope = stdout_json(&output);
    assert_eq!(envelope["status"], "valid");
    assert_eq!(envelope["operation"], "init");
    assert_eq!(envelope["mode"], "dry-run");
    assert_eq!(envelope["changed"], false);
    assert_eq!(envelope["projectId"], "acme_node_monorepo");
    assert_eq!(envelope["projectIdSource"]["source"], "package.json#name");
    assert_eq!(
        envelope["projectIdSource"]["original"],
        "acme-node-monorepo"
    );
    assert_eq!(envelope["target"], "node-typescript");
    let writes = envelope["writes"].as_array().expect("writes array");
    assert_eq!(writes.len(), 2);
    let paths: Vec<&str> = writes
        .iter()
        .map(|write| write["path"].as_str().expect("write path"))
        .collect();
    assert_eq!(
        paths,
        vec!["lekalo/project.yaml", "lekalo/targets/node-typescript.yaml"]
    );
    for write in writes {
        assert_eq!(write["action"], "create");
        assert_eq!(write["disposition"], "create");
    }
    let detection = &envelope["detection"];
    assert_eq!(detection["basis"], "workspace-root");
    let workspace_roots = detection["workspaceRoots"]
        .as_array()
        .expect("workspace roots");
    assert_eq!(workspace_roots.len(), 1);
    assert_eq!(workspace_roots[0]["path"], ".");
    assert_eq!(workspace_roots[0]["ecosystem"], "node");
    assert_eq!(workspace_roots[0]["members"][0], "apps/*");
    let modules = detection["modules"].as_array().expect("observed modules");
    let module_paths: Vec<&str> = modules
        .iter()
        .map(|module| module["path"].as_str().expect("module path"))
        .collect();
    assert_eq!(module_paths, vec!["apps/web", "packages/ui"]);
    for module in modules {
        assert_eq!(module["mode"], "observed");
        assert_eq!(module["confidence"], "high");
    }
    let layouts = detection["layouts"].as_array().expect("layouts");
    assert!(layouts
        .iter()
        .any(|layout| layout["kind"] == "openspec" && layout["confidence"] == "high"));
    let gates = detection["gates"].as_array().expect("gates");
    assert!(gates.iter().any(|gate| gate["gate"] == "build"
        && gate["source"] == "workflow"
        && gate["command"] == "npm run build"));
    assert!(detection["packageManagers"]
        .as_array()
        .expect("package managers")
        .iter()
        .any(|item| item["detail"] == "npm"));
    // Dry-run wrote nothing at all.
    assert_eq!(snapshot(&root, false), before);
}

/// The applied adoption writes only the minimal canonical skeleton, keeps
/// every `apps/**` and `packages/**` byte identical, and the skeleton
/// passes the normal loader and validator.
#[test]
fn adopt_creates_minimal_skeleton_and_preserves_the_monorepo() {
    let (_temp, root) = materialize("tests/fixtures/adopt/node-monorepo");
    let before = snapshot(&root, true);
    let output = lekalo_in(&root, &["--json", "init", "--adopt"]);
    assert_eq!(exit_code(&output), 0, "{}", stderr_text(&output));
    let envelope = stdout_json(&output);
    assert_eq!(envelope["mode"], "apply");
    assert_eq!(envelope["changed"], true);
    assert_eq!(envelope["created"], 1);
    assert_eq!(envelope["gate"]["status"], "valid");
    assert_eq!(envelope["gate"]["modelVersion"], "0.2.16");
    // The canonical skeleton is exactly the project document.
    let project_yaml = fs::read(root.join("lekalo").join("project.yaml")).expect("skeleton bytes");
    assert_eq!(
        project_yaml,
        b"{\"schema_version\":\"0.2.16\",\"definitions\":[{\"id\":\"acme_node_monorepo\",\"kind\":\"project\",\"version\":1,\"description\":\"Adopted existing project.\"}]}\n"
    );
    // No `apps/**` or `packages/**` byte changed; only `lekalo/**` appeared.
    assert_eq!(snapshot(&root, true), before);
    // The written skeleton loads and validates through the normal path.
    let load = lekalo_in(&root, &["--json", "load"]);
    assert_eq!(exit_code(&load), 0, "{}", stderr_text(&load));
    let validate = lekalo_in(&root, &["--json", "validate"]);
    assert_eq!(exit_code(&validate), 0, "{}", stderr_text(&validate));
    assert_eq!(stdout_json(&validate)["status"], "valid");
}

/// A Cargo workspace can contain modules from all four external discovery
/// packages. The host still owns scope, path validation and read-only adoption.
#[test]
fn external_language_discovery_keeps_mixed_workspace_members_confined() {
    let temp = tempfile::tempdir().expect("temp");
    let root = temp.path().join("project");
    fs::create_dir_all(&root).unwrap();
    let files = [
        ("Cargo.toml", "[package]\nname = \"workspace_service\"\n[workspace]\nmembers = [\"components/*\", \"../outside\"]\n"),
        ("components/api/go.mod", "module example.com/team/api\n"),
        ("components/native/Cargo.toml", "[package]\nname = \"native_service\"\n"),
        ("components/web/package.json", r#"{"name":"web","dependencies":{"react":"1"},"devDependencies":{"typescript":"1"},"scripts":{"test":"touch executed"}}"#),
        ("components/worker/pyproject.toml", "[project]\nname = 'worker'\n"),
        ("components/worker/setup.py", "raise RuntimeError('must never execute')\n"),
    ];
    for (path, text) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fs::create_dir_all(temp.path().join("outside")).unwrap();
    fs::write(
        temp.path().join("outside/Cargo.toml"),
        "[package]\nname = \"outside_marker\"\n",
    )
    .unwrap();
    let before = snapshot(&root, false);
    let output = lekalo_in(&root, &["--json", "init", "--adopt", "--dry-run"]);
    assert_eq!(exit_code(&output), 0, "{}", stderr_text(&output));
    let envelope = stdout_json(&output);
    assert_eq!(envelope["projectId"], "workspace_service");
    let detection = &envelope["detection"];
    let modules = detection["modules"].as_array().unwrap();
    let names: Vec<_> = modules
        .iter()
        .map(|module| module["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["api", "native_service", "web", "components/worker"]);
    assert!(detection["manifests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["path"] == "components/worker/setup.py" && entry["detail"] == "python"));
    assert!(detection["languages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|hint| hint["label"] == "react" && hint["kind"] == "framework"));
    assert!(!stdout_text(&output).contains("outside_marker"));
    assert_eq!(
        snapshot(&root, false),
        before,
        "discovery cannot execute scripts or write files"
    );
}
