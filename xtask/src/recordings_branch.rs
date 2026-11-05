use std::{env, error::Error, path::Path, process::ExitCode};

use git2::{
    Cred, FetchOptions, IndexAddOption, Oid, PushOptions, RemoteCallbacks, Repository,
    RepositoryInitOptions, Signature,
};

const RECORDINGS_DIRECTORY: &str = "recordings";
const RECORDINGS_BRANCH_REFERENCE: &str = "refs/heads/recordings";
const PUBLISHED_TRACKING_REFERENCE: &str = "refs/remotes/origin/recordings";
const CI_AUTHOR_NAME: &str = "sottomano-ci";
const CI_AUTHOR_EMAIL: &str = "willemvanhulle@protonmail.com";
const CODEBERG_USER: &str = "wvhulle-bot";

fn required_environment(variable_name: &str) -> Result<String, Box<dyn Error>> {
    env::var(variable_name).map_err(|missing| format!("{variable_name}: {missing}").into())
}

fn token_callbacks(codeberg_token: &str) -> RemoteCallbacks<'_> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, _username, _allowed| {
        Cred::userpass_plaintext(CODEBERG_USER, codeberg_token)
    });
    callbacks
}

fn committed_recordings_tree(
    repository: &Repository,
    commit_message: &str,
) -> Result<Oid, git2::Error> {
    let mut index = repository.index()?;
    index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let tree_id = index.write_tree()?;
    let tree = repository.find_tree(tree_id)?;
    let signature = Signature::now(CI_AUTHOR_NAME, CI_AUTHOR_EMAIL)?;
    repository.commit(
        Some("HEAD"),
        &signature,
        &signature,
        commit_message,
        &tree,
        &[],
    )?;
    Ok(tree_id)
}

fn published_tree(
    repository: &Repository,
    remote_url: &str,
    codeberg_token: &str,
) -> Result<Oid, git2::Error> {
    let mut fetch_options = FetchOptions::new();
    fetch_options
        .remote_callbacks(token_callbacks(codeberg_token))
        .depth(1);
    repository.remote_anonymous(remote_url)?.fetch(
        &[format!(
            "+{RECORDINGS_BRANCH_REFERENCE}:{PUBLISHED_TRACKING_REFERENCE}"
        )],
        Some(&mut fetch_options),
        None,
    )?;
    let published_commit = repository
        .find_reference(PUBLISHED_TRACKING_REFERENCE)?
        .peel_to_commit()?;
    Ok(published_commit.tree_id())
}

fn force_pushed_recordings(
    repository: &Repository,
    remote_url: &str,
    codeberg_token: &str,
) -> Result<(), Box<dyn Error>> {
    let mut rejections = Vec::new();
    let mut callbacks = token_callbacks(codeberg_token);
    callbacks.push_update_reference(|reference, rejection| {
        if let Some(reason) = rejection {
            rejections.push(format!("{reference}: {reason}"));
        }
        Ok(())
    });
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);
    repository.remote_anonymous(remote_url)?.push(
        &[format!(
            "+{RECORDINGS_BRANCH_REFERENCE}:{RECORDINGS_BRANCH_REFERENCE}"
        )],
        Some(&mut push_options),
    )?;
    drop(push_options);
    if rejections.is_empty() {
        Ok(())
    } else {
        Err(format!("push rejected: {}", rejections.join(", ")).into())
    }
}

pub fn publish_recordings(workspace_root: &Path) -> Result<ExitCode, Box<dyn Error>> {
    let commit_sha = required_environment("CI_COMMIT_SHA")?;
    let repository_name = required_environment("CI_REPO")?;
    let codeberg_token = required_environment("CODEBERG_TOKEN")?;
    let remote_url = format!("https://codeberg.org/{repository_name}.git");
    let repository = Repository::init_opts(
        workspace_root.join(RECORDINGS_DIRECTORY),
        RepositoryInitOptions::new().initial_head(RECORDINGS_BRANCH_REFERENCE),
    )?;
    let rendered_tree = committed_recordings_tree(
        &repository,
        &format!("docs: README recordings of {commit_sha}"),
    )?;
    if published_tree(&repository, &remote_url, &codeberg_token)
        .is_ok_and(|published| published == rendered_tree)
    {
        println!("recordings unchanged");
        return Ok(ExitCode::SUCCESS);
    }
    force_pushed_recordings(&repository, &remote_url, &codeberg_token)?;
    println!("published recordings of {commit_sha} to the recordings branch");
    Ok(ExitCode::SUCCESS)
}
