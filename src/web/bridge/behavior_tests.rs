use std::io::Write;
use std::process::{Command, Stdio};

/// Exercises the actual generated page with deterministic network and native-boundary doubles.
pub(super) fn run(page: &super::BridgePage) {
    // The regression executes the generated page in Node.js 22+; skip it where the
    // toolchain is absent instead of failing unrelated builds.
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipping WEB bridge JS regression: Node.js 22+ is not installed");
        return;
    }
    let mut child = Command::new("node")
        .args(["-e", include_str!("behavior_tests.js"), "--", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Node.js 22+ is required for executable WEB bridge regressions");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(page.body.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "bridge regressions failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
