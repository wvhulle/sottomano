#!/usr/bin/env nu

def parse-hibernate-delay [requested_instance: string, allowed_delays: record<shortest: duration, longest: duration>]: nothing -> duration {
  if ($requested_instance !~ '^[1-9][0-9]{0,5}$') {
    error make {
      msg: $"hibernate delay must be whole seconds, got: ($requested_instance)"
      label: {text: "unit instance", span: (metadata $requested_instance).span}
    }
  }
  let hibernate_delay = $"($requested_instance)sec" | into duration
  if $hibernate_delay < $allowed_delays.shortest or $hibernate_delay > $allowed_delays.longest {
    error make {
      msg: $"hibernate delay ($hibernate_delay) is outside ($allowed_delays.shortest) to ($allowed_delays.longest)"
      label: {text: "unit instance", span: (metadata $requested_instance).span}
    }
  }
  $hibernate_delay
}

def main [
  requested_instance: string
  drop_in_path: path
  --shortest-delay: duration = 1sec
  --longest-delay: duration = 1day
]: nothing -> nothing {
  let hibernate_delay = parse-hibernate-delay $requested_instance {shortest: $shortest_delay, longest: $longest_delay}
  let delay_seconds = $hibernate_delay / 1sec | into int
  let staging_path = $"($drop_in_path).staging"
  try {
    mkdir ($drop_in_path | path dirname)
    $"[Sleep]\nHibernateDelaySec=($delay_seconds)s\n" | save --force $staging_path
    mv --force $staging_path $drop_in_path
  } catch {|write_error|
    error make {
      msg: $"cannot write the sleep drop-in: ($write_error.msg)"
      label: {text: "drop-in path", span: (metadata $drop_in_path).span}
    }
  }
  print $"set the hibernate delay to ($hibernate_delay) in ($drop_in_path)"
}
