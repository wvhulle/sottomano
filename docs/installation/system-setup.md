# System setup without NixOS

The [NixOS module](nix.md#system-module) enables the system pieces some widgets
read. Without it, `sottomano setup` sets them up by hand:

```bash
sudo sottomano setup linux
```

`sottomano setup linux --print` shows every file it writes and every command it
runs first. On Ubuntu and Fedora the [distribution
subcommands](ubuntu.md#system-setup) also install the runtime tools. What each
piece is for:

- A `drivetemp` module load at boot gives the SATA drive temperatures.
- A udev rule makes the RAPL energy counters world-readable for the CPU power
  reading, and hands the `amd_x3d_mode` sysfs file to the administrators group
  (`wheel` or `sudo`, auto-detected) so the bar can switch the X3D cache or
  frequency preference. The RAPL rule can be left out with `--skip rapl`; it is
  the PLATYPUS power side channel, so skip it on machines shared with
  untrusted users.
- The `amd_uncore` module load and `kernel.perf_event_paranoid = 0` (together
  the `amd-uncore` step) give the per-chiplet L3 hit rates; `--skip amd-uncore`
  leaves them out.
- Adding your user to `systemd-journal` (`--skip journal` leaves it out) lets
  the journal group read the system journal.

The runtime tools and D-Bus services the widgets call come from each
distribution's package manager — NetworkManager and its VPN plugins,
ModemManager, bluez, PipeWire with wireplumber, fwupd, UPower and a
power-profiles-daemon implementation (Fedora's `tuned-ppd` serves the same
D-Bus API). Tailscale comes from
[its own repository](https://tailscale.com/download/linux).