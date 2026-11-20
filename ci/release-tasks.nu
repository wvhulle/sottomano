#!/usr/bin/env nu

const CODEBERG_USER = "wvhulle-bot"

def run-release-plz [subcommand: string]: nothing -> nothing {
  ^release-plz $subcommand --forge gitea --git-token $env.CODEBERG_TOKEN
}

def "main authenticate-git-origin" []: nothing -> nothing {
  ^git remote set-url origin $"https://($CODEBERG_USER):($env.CODEBERG_TOKEN)@codeberg.org/($env.CI_REPO).git"
}

def "main publish-unreleased-crate-version" []: nothing -> nothing {
  main authenticate-git-origin
  ^git checkout -B main $env.CI_COMMIT_SHA
  ^git fetch origin main:refs/remotes/origin/main
  ^git branch --set-upstream-to=origin/main main
  run-release-plz release
}

def "main open-or-update-release-pr" []: nothing -> nothing {
  run-release-plz release-pr
}

def "main check-commit-messages" [
  --fetch-target-branch
  ...revision_ranges: string
]: nothing -> nothing {
  if $fetch_target_branch {
    ^git fetch --quiet origin $env.CI_COMMIT_TARGET_BRANCH
  }
  ^convco check ...$revision_ranges
}

def "main check-release-pr-is-current" []: nothing -> nothing {
  if (^git rev-parse --is-shallow-repository | str trim) == "true" {
    ^git fetch --quiet --unshallow origin
  }
  ^git fetch --quiet origin $env.CI_COMMIT_TARGET_BRANCH
  let target_branch_head = ^git rev-parse FETCH_HEAD | str trim
  let release_commit_parents = ^git rev-list --parents --max-count=1 HEAD | str trim | split row " " | skip 1
  if $release_commit_parents != [$target_branch_head] {
    error make {
      msg: $"The release pull request is not a single commit on top of ($env.CI_COMMIT_TARGET_BRANCH) at ($target_branch_head), so its changelog misses newer commits. Wait for open-or-update-release-pr to refresh it, or start the release-pr workflow manually."
    }
  }
}

def main []: nothing -> nothing {
  print "Run a subcommand, for example: check-commit-messages"
}
