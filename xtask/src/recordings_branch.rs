use std::{
    env,
    error::Error,
    path::Path,
    process::{Command, ExitCode},
};

const RECORDINGS_DIRECTORY: &str = "recordings";
const RECORDINGS_BRANCH: &str = "recordings";
const CI_AUTHOR_NAME: &str = "sottomano-ci";
const CI_AUTHOR_EMAIL: &str = "willemvanhulle@protonmail.com";
const CODEBERG_USER: &str = "wvhulle";

fn required_environment(variable_name: &str) -> Result<String, Box<dyn Error>> {
    env::var(variable_name).map_err(|missing| format!("{variable_name}: {missing}").into())
}

fn git_output(recordings_path: &Path, arguments: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(recordings_path)
        .args(arguments)
        .env("GIT_AUTHOR_NAME", CI_AUTHOR_NAME)
        .env("GIT_AUTHOR_EMAIL", CI_AUTHOR_EMAIL)
        .env("GIT_COMMITTER_NAME", CI_AUTHOR_NAME)
        .env("GIT_COMMITTER_EMAIL", CI_AUTHOR_EMAIL)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn published_tree(recordings_path: &Path) -> Option<String> {
    git_output(
        recordings_path,
        &[
            "fetch",
            "--quiet",
            "--depth",
            "1",
            "origin",
            RECORDINGS_BRANCH,
        ],
    )
    .and_then(|_| git_output(recordings_path, &["rev-parse", "FETCH_HEAD^{tree}"]))
    .ok()
}

pub fn publish_recordings(workspace_root: &Path) -> Result<ExitCode, Box<dyn Error>> {
    let commit_sha = required_environment("CI_COMMIT_SHA")?;
    let repository = required_environment("CI_REPO")?;
    let codeberg_token = required_environment("CODEBERG_TOKEN")?;
    let recordings_path = workspace_root.join(RECORDINGS_DIRECTORY);
    let commit_message = format!("docs: README recordings of {commit_sha}");
    let remote_url =
        format!("https://{CODEBERG_USER}:{codeberg_token}@codeberg.org/{repository}.git");
    git_output(
        &recordings_path,
        &["init", "--quiet", "--initial-branch", RECORDINGS_BRANCH],
    )?;
    git_output(&recordings_path, &["add", "--all"])?;
    git_output(
        &recordings_path,
        &["commit", "--quiet", "--message", &commit_message],
    )?;
    git_output(&recordings_path, &["remote", "add", "origin", &remote_url])?;
    let rendered_tree = git_output(&recordings_path, &["rev-parse", "HEAD^{tree}"])?;
    if published_tree(&recordings_path).is_some_and(|published| published == rendered_tree) {
        println!("recordings unchanged");
        return Ok(ExitCode::SUCCESS);
    }
    git_output(
        &recordings_path,
        &["push", "--quiet", "--force", "origin", RECORDINGS_BRANCH],
    )?;
    println!("published recordings of {commit_sha} to the {RECORDINGS_BRANCH} branch");
    Ok(ExitCode::SUCCESS)
}
