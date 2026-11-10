# System setup without NixOS

The [NixOS module](nix.md#system-module) enables the system pieces some widgets
read. Without it, set them up by hand. The concrete commands are on each
distribution's page — [Ubuntu](ubuntu.md#system-setup) and
[Fedora](fedora.md#system-setup) — and the same steps work on other
distributions with their package manager's equivalents.

- A `drivetemp` module load at boot gives the SATA drive temperatures.
- A udev rule makes the RAPL energy counters world-readable for the CPU power
  reading, and hands the `amd_x3d_mode` sysfs file to the administrators group
  (`wheel` or `sudo`) so the bar can switch the X3D cache or frequency
  preference. Leave out the RAPL rule on machines shared with untrusted users:
  it is the PLATYPUS power side channel.
- On AMD, an `amd_uncore` module load and
  `kernel.perf_event_paranoid = 0` give the per-chiplet L3 hit rates.
- Adding your user to `systemd-journal` lets the journal group read the system
  journal.

The runtime tools and D-Bus services the widgets call are on each
[distribution page](ubuntu.md#runtime-tools-and-services): NetworkManager and
its VPN plugins, ModemManager, bluez, PipeWire with wireplumber, fwupd, UPower
and a power-profiles-daemon implementation (Fedora's `tuned-ppd` serves the
same D-Bus API). Tailscale comes from
[its own repository](https://tailscale.com/download/linux).