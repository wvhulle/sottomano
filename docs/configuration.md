# Configuration

There is one TOML file, and the shell prints its complete default itself, so
there is no option reference here to keep in sync.

## The config file

The file is `$XDG_CONFIG_HOME/sottomano.toml`. With home-manager, set
`services.sottomano.settings` and it is written to that file verbatim.
`sottomano --print-default-config` prints every key with its default, and
unknown keys are rejected. The shell applies every change to the file as it
runs. An invalid file raises a toast and the previous configuration stays in
use.

Temperature sensors are detected automatically, so their keys don't appear in
the printed defaults. To pin a sensor or override a threshold:

```toml
[temperature.cpu]
sensor = { chip = "k10temp", label = "Tctl" }
critical_celsius = 90
```

## Secrets

A few widgets need an API key. Each key is optional, and the shell works
without them.

`sottomano --list-secrets` lists the optional secret files, what each one
enables and whether it is present. `services.sottomano.secretFiles.<name>`
symlinks each one from wherever the secret actually lives, so nothing lands in
the nix store.
