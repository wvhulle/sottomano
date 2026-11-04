# Compute

The machine's processor: load, clock, temperature, power and the cache layout of each chiplet.

<table>
<tr><th>In its menu</th><th>Pinned in the bar</th></tr>
<tr>
<td width="50%" valign="top"><img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/compute_cpu.webp" alt="Opening the compute group from the Hardware menu" width="100%"></td>
<td width="50%" valign="top"><img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/compute_cpu_pinned.webp" alt="The compute group pinned in the panel, then expanded" width="100%"></td>
</tr>
</table>

## Compute: CPU

Pressing the compute row in the Hardware menu opens load, temperature, power and model.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/compute_cpu.webp" width="1260" alt="Opening the compute group from the Hardware menu from the CPU reading">

## Compute: Model

The CPU model with its vendor logo, microarchitecture and caches.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/compute_model.webp" width="1108" alt="Hovering the CPU model in the compute group">

## Compute: Other readings

These open from the same row and look like the readings above.

| Reading | What it shows |
|---|---|
| Clock | Each thread's clock per chiplet, with the chiplet the scheduler prefers starred. |
| Temperature | The hottest sensor, and the chip it is read from. |
| Cache preference | On dual-chiplet X3D parts, which chiplet the scheduler fills first; a click flips it. |
