#[cfg(test)]
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn ck_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ck"))
}

fn ck_command() -> Command {
    let mut cmd = Command::new(ck_binary());
    cmd.env_remove("CK_INDEX_DIR");
    cmd
}

#[test]
fn test_basic_grep_functionality() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("file1.txt"),
        "hello world\nrust programming\ntest line",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("file2.rs"),
        "fn main() {\n    println!(\"Hello Rust\");\n}",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("file3.py"),
        "print('Hello Python')\n# rust comment",
    )
    .unwrap();

    let output = ck_command()
        .args(["rust", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("rust programming"));
    assert!(stdout.contains("# rust comment"));
}

#[test]
fn test_case_insensitive_search() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("test.txt"),
        "Hello World\nHELLO WORLD\nhello world",
    )
    .unwrap();

    let output = ck_command()
        .args(["-i", "HELLO", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 3);
}

#[test]
fn test_recursive_search() {
    let temp_dir = TempDir::new().unwrap();
    fs::create_dir(temp_dir.path().join("subdir")).unwrap();
    fs::write(temp_dir.path().join("root.txt"), "target text").unwrap();
    fs::write(
        temp_dir.path().join("subdir").join("nested.txt"),
        "target text",
    )
    .unwrap();

    let output = ck_command()
        .args(["-r", "target", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 2);
}

#[test]
fn test_json_output() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.txt"), "json test line").unwrap();

    let output = ck_command()
        .args(["--json", "json", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    let json_result: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(json_result.is_array());
    assert!(json_result[0]["file"].is_string());
    assert!(json_result[0]["score"].is_number());
    assert!(json_result[0]["preview"].is_string());
}

#[test]
fn test_context_lines() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("context.txt"),
        "line 1\nline 2\ntarget line\nline 4\nline 5",
    )
    .unwrap();

    let output = ck_command()
        .args(["-C", "1", "target", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck with context");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("line 2"));
    assert!(stdout.contains("target line"));
    assert!(stdout.contains("line 4"));
}

#[test]
fn test_topk_limit() {
    let temp_dir = TempDir::new().unwrap();

    for i in 1..=10 {
        fs::write(
            temp_dir.path().join(format!("file{i}.txt")),
            "match content",
        )
        .unwrap();
    }

    let output = ck_command()
        .args(["--topk", "5", "match", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck with topk");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.trim().lines().count();
    assert!(line_count <= 10);
}

#[test]
fn test_line_numbers() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("numbered.txt"),
        "line 1\nmatched line\nline 3",
    )
    .unwrap();

    let output = ck_command()
        .args(["-n", "matched", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck with line numbers");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("2:matched line"));
}

#[test]
fn test_no_matches_stderr_message() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.txt"), "hello world").unwrap();

    let output = ck_command()
        .args(["nonexistent_pattern", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().is_empty());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("No matches found"));
}

#[test]
fn test_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();

    let output = ck_command()
        .args(["[invalid", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(!output.status.success());
}

#[test]
fn test_invalid_regex_warning_during_highlighting() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("test.txt"),
        "hello world\ntest content",
    )
    .unwrap();

    let output = ck_command()
        .args([
            "[invalid",
            temp_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run ck");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("regex") || stderr.contains("pattern") || stderr.contains("invalid"));
}

#[test]
fn test_jsonl_basic_output() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("test.rs"),
        "fn main() {\n    println!(\"Hello Rust\");\n}",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("test.py"),
        "print('Hello Python')\ndef main():\n    pass",
    )
    .unwrap();

    let output = ck_command()
        .args(["fn main", "--jsonl", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.trim().is_empty());

    for line in stdout.lines() {
        if !line.trim().is_empty() {
            let json: serde_json::Value = serde_json::from_str(line).expect("Invalid JSON line");
            assert!(json.get("path").is_some());
            assert!(json.get("span").is_some());
            assert!(json.get("language").is_some());
            assert!(json.get("snippet").is_some());
            assert!(json.get("score").is_some());

            let span = json.get("span").unwrap().as_object().unwrap();
            assert!(span.get("byte_start").is_some());
            assert!(span.get("byte_end").is_some());
            assert!(span.get("line_start").is_some());
            assert!(span.get("line_end").is_some());
        }
    }
}

#[test]
fn test_jsonl_no_snippet_flag() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("test.rs"),
        "fn main() {\n    println!(\"Hello Rust\");\n}",
    )
    .unwrap();

    let output = ck_command()
        .args([
            "fn main",
            "--jsonl",
            "--no-snippet",
            temp_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    for line in stdout.lines() {
        if !line.trim().is_empty() {
            let json: serde_json::Value = serde_json::from_str(line).expect("Invalid JSON line");
            assert!(json.get("snippet").is_none());
            assert!(json.get("path").is_some());
            assert!(json.get("span").is_some());
            assert!(json.get("language").is_some());
            assert!(json.get("score").is_some());
        }
    }
}

#[test]
fn test_jsonl_vs_regular_output() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("test.rs"),
        "fn main() {\n    println!(\"Hello Rust\");\n}",
    )
    .unwrap();

    let regular_output = ck_command()
        .args(["fn main", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    let jsonl_output = ck_command()
        .args(["fn main", "--jsonl", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(regular_output.status.success());
    assert!(jsonl_output.status.success());

    let regular_stdout = String::from_utf8(regular_output.stdout).unwrap();
    let jsonl_stdout = String::from_utf8(jsonl_output.stdout).unwrap();

    assert!(!regular_stdout.contains("{\"path\":"));
    assert!(jsonl_stdout.contains("{\"path\":"));
    assert!(jsonl_stdout.contains("\"span\":"));
    assert!(jsonl_stdout.contains("\"language\":"));
}

#[test]
fn test_no_ckignore_flag_disables_hierarchical_ignore() {
    let temp_dir = TempDir::new().unwrap();
    let parent = temp_dir.path();
    let subdir = parent.join("subdir");
    fs::create_dir(&subdir).unwrap();

    fs::write(parent.join(".ckignore"), "*.tmp\n").unwrap();
    fs::write(parent.join("test.txt"), "FINDME_TEXT").unwrap();
    fs::write(parent.join("ignored.tmp"), "FINDME_TMP").unwrap();
    fs::write(subdir.join("nested.txt"), "FINDME_TEXT").unwrap();
    fs::write(subdir.join("also_ignored.tmp"), "FINDME_TMP").unwrap();

    let output = ck_command()
        .args(["-r", "--no-ckignore", "FINDME", "."])
        .current_dir(parent)
        .output()
        .expect("Failed to run ck search --no-ckignore");

    assert!(
        output.status.success(),
        "Search with --no-ckignore failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        stdout.contains("ignored.tmp") || stdout.contains("also_ignored.tmp"),
        "Should find .tmp files when --no-ckignore is used. Output: {stdout}"
    );

    let output = ck_command()
        .args(["-r", "FINDME", "."])
        .current_dir(parent)
        .output()
        .expect("Failed to run ck search");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        !stdout.contains("ignored.tmp") && !stdout.contains("also_ignored.tmp"),
        "Should NOT find .tmp files when .ckignore is active (default). Output: {stdout}"
    );

    assert!(
        stdout.contains("test.txt") || stdout.contains("nested.txt"),
        "Should find .txt files even with .ckignore active"
    );
}

#[cfg(unix)]
#[test]
fn test_sigpipe_terminates_silently() {
    use std::io::Read;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Stdio;

    let temp_dir = TempDir::new().unwrap();
    let line = "match: the quick brown fox jumps over the lazy dog\n";
    fs::write(temp_dir.path().join("big.txt"), line.repeat(50_000)).unwrap();

    let mut child = ck_command()
        .args(["match", temp_dir.path().to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn ck");

    let mut stdout = child.stdout.take().unwrap();
    let mut buf = [0u8; 4096];
    let _ = stdout.read(&mut buf).unwrap();
    drop(stdout);

    let status = child.wait().expect("Failed to wait on ck");
    let mut stderr_out = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr_out)
        .unwrap();

    assert!(
        !stderr_out.contains("panicked"),
        "ck panicked on broken pipe: {stderr_out}"
    );
    assert_eq!(
        status.signal(),
        Some(libc::SIGPIPE),
        "ck should be terminated by SIGPIPE, got status {status:?}"
    );
}

fn setup_hidden_workspace() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("visible.txt"),
        "needle in visible file",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join(".hidden-file.txt"),
        "needle in hidden file",
    )
    .unwrap();
    let hidden_dir = temp_dir.path().join(".hidden-dir");
    fs::create_dir(&hidden_dir).unwrap();
    fs::write(hidden_dir.join("inside.txt"), "needle inside hidden dir").unwrap();
    temp_dir
}

#[test]
fn test_hidden_flag_regex_search() {
    let temp_dir = setup_hidden_workspace();

    let output = ck_command()
        .args(["needle", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck search");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("visible.txt"),
        "visible file should always match; stdout: {stdout}"
    );
    assert!(
        !stdout.contains(".hidden-file.txt"),
        "hidden file should NOT match without --hidden; stdout: {stdout}"
    );
    assert!(
        !stdout.contains("inside.txt"),
        "file in hidden dir should NOT match without --hidden; stdout: {stdout}"
    );

    let output = ck_command()
        .args(["--hidden", "needle", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck --hidden search");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("visible.txt"),
        "visible file should still match with --hidden; stdout: {stdout}"
    );
    assert!(
        stdout.contains(".hidden-file.txt"),
        "hidden file SHOULD match with --hidden; stdout: {stdout}"
    );
    assert!(
        stdout.contains("inside.txt"),
        "file in hidden dir SHOULD match with --hidden; stdout: {stdout}"
    );
}
