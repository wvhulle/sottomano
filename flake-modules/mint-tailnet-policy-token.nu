#!/usr/bin/env nu

def read-oauth-credentials [credential_file: path]: nothing -> record {
  let credential_text = try {
    open --raw $credential_file
  } catch {|open_error|
    error make {
      msg: $"cannot read the OAuth client credential: ($open_error.msg)"
      label: {text: "credential file", span: (metadata $credential_file).span}
    }
  }
  $credential_text
  | lines
  | where $it =~ '='
  | each {|credential_line|
    let assignment_parts = $credential_line | split row --number 2 '='
    [($assignment_parts | first | str trim) ($assignment_parts | last | str trim)]
  }
  | into record
}

def request-policy-access-token [oauth_credentials: record]: nothing -> string {
  let token_response = try {
    http post --content-type application/x-www-form-urlencoded --max-time 30sec https://api.tailscale.com/api/v2/oauth/token {
      client_id: $oauth_credentials.TAILSCALE_OAUTH_CLIENT_ID
      client_secret: $oauth_credentials.TAILSCALE_OAUTH_CLIENT_SECRET
      grant_type: client_credentials
    }
  } catch {|request_error|
    error make {
      msg: $"the Tailscale OAuth token request failed: ($request_error.msg)"
      label: {text: "OAuth client", span: (metadata $oauth_credentials).span}
    }
  }
  let access_token = $token_response.access_token?
  if ($access_token | is-empty) {
    error make {
      msg: "the Tailscale OAuth response contains no access_token"
      label: {text: "OAuth client", span: (metadata $oauth_credentials).span}
    }
  }
  $access_token
}

def main [credential_file: path, token_path: path]: nothing -> nothing {
  let access_token = request-policy-access-token (read-oauth-credentials $credential_file)
  let staging_path = $"($token_path).staging"
  try {
    $access_token | save --force $staging_path
    mv --force $staging_path $token_path
  } catch {|write_error|
    error make {
      msg: $"cannot write the token file: ($write_error.msg)"
      label: {text: "token path", span: (metadata $token_path).span}
    }
  }
  print $"refreshed the read-only tailnet policy token in ($token_path)"
}
