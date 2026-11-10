# Session startup

Outside home-manager, write the user service the module would generate to
`~/.config/systemd/user/sottomano.service`:

```ini
[Unit]
Description=sottomano bar
PartOf=graphical-session.target
After=graphical-session.target

[Service]
ExecStart=%h/.cargo/bin/sottomano
Restart=on-failure
RestartSec=2
Slice=session.slice
CPUWeight=500

[Install]
WantedBy=graphical-session.target
```

Use `%h/.nix-profile/bin/sottomano` for a Nix profile install, then enable it:

```bash
systemctl --user enable --now sottomano.service
```

`session.slice` and the raised weight keep the bar responsive when apps load the
machine. The service starts with `graphical-session.target`. niri starts that
target itself; sway and Hyprland need a line in their config, as
[their pages](compositors/sway.md#start-with-the-session) show.

Next: [configuration](configuration.md).