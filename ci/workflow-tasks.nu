#!/usr/bin/env nu

export def copy-tree-writable [
  source_directory: path
  target_directory: path
]: nothing -> nothing {
  mkdir $target_directory
  for source_entry in (ls --all ($source_directory | path expand)) {
    cp --recursive --preserve [] $source_entry.name $target_directory
  }
}

def "main collect-rendered-recordings" [
  build_result: path
  target_directory: path
]: nothing -> nothing {
  copy-tree-writable $build_result $target_directory
}

def "main check-commits" [
  --fetch-target-branch
  ...revision_ranges: string
]: nothing -> nothing {
  if $fetch_target_branch {
    ^git fetch --quiet origin $env.CI_COMMIT_TARGET_BRANCH
  }
  ^convco check ...$revision_ranges
}

def main []: nothing -> nothing {
  print "Run a subcommand, for example: check-commits"
}
