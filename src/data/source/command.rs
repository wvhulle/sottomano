use std::process::Command;

pub fn output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn run(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .status()
        .is_ok_and(|status| status.success())
}

pub fn exists(program: &str) -> bool {
    output("sh", &["-c", &format!("command -v {program}")]).is_some()
}

pub fn http_get(url: &str, timeout_secs: u32) -> Option<String> {
    output(
        "curl",
        &["-s", "--max-time", &timeout_secs.to_string(), url],
    )
    .filter(|body| !body.trim().is_empty())
}
