#!/usr/bin/env nu

use std/assert
use ./nix-built-paths.nu [nix-config-with-hook store-paths-from-lines]
use ./workflow-tasks.nu [copy-tree-writable]

def assert-copy-becomes-writable []: nothing -> nothing {
  let scratch_directory = mktemp --directory
  let source_directory = $scratch_directory | path join source
  let target_directory = $scratch_directory | path join target
  let source_subdirectory = $source_directory | path join recordings
  mkdir $source_subdirectory
  "rendered" | save ($source_subdirectory | path join demo.webp)
  "hidden" | save ($source_directory | path join .hidden-marker)
  ^chmod -R a-w $source_directory

  copy-tree-writable $source_directory $target_directory

  let copied_recording = $target_directory | path join recordings demo.webp
  assert equal (open --raw $copied_recording) rendered
  assert (($target_directory | path join .hidden-marker) | path exists)
  assert ((ls --long $copied_recording | first | get mode) starts-with rw)
  assert ((ls --long --directory ($target_directory | path join recordings) | first | get mode) starts-with rwx)

  ^chmod -R u+w $source_directory
  rm --recursive --force $scratch_directory
}

def main []: nothing -> nothing {
  assert equal (nix-config-with-hook "max-jobs = 1" "/nix/store/hook") "max-jobs = 1\npost-build-hook = /nix/store/hook"
  assert equal (nix-config-with-hook "" "/nix/store/hook") "\npost-build-hook = /nix/store/hook"
  assert equal ("/nix/store/first\n\n/nix/store/second\n" | store-paths-from-lines) ["/nix/store/first" "/nix/store/second"]
  assert equal ("" | store-paths-from-lines) []
  assert-copy-becomes-writable
  print "CI task tests passed"
}
