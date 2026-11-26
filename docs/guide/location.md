# Location

Where [situo](https://codeberg.org/wvhulle/situo) places you, and how. The weather readings use this position. Each source refreshes when your own grid cell changes: about 1 km for Open-Meteo and about 100 m for Buienradar's radar.

The group sits in the Agenda menu. Nothing from it shows on the panel unless you pin an item, or the update status turns red because situo's lookups are failing. The refresh button in any of its cards asks situo for a new lookup.

<div class="beside">
<div class="beside-text">

Hovering the estimate shows which source decided the fix, and what Wi-Fi and the serving cell each look up to on their own, with how far apart they are.

</div>

<img src="https://sottomano.work/recordings/location_fix.webp" srcset="https://sottomano.work/recordings/location_fix.webp 2x" alt="Opening the location group from the Agenda menu and hovering the estimate">

</div>

| Item | Panel label | Card |
| --- | --- | --- |
| Place | the nearest place | the place, the position and whether it was configured, pinned or looked up, with links to a map |
| Estimate | BeaconDB's accuracy | which source decided the fix, and what Wi-Fi and the serving cell each look up to on their own, with how far apart they are |
| Radio | access points sent out of seen | what the Wi-Fi scan and the modems reported, the access points situo held back and why, and the serving cells sent |
| Updates | the last lookup | whether the last lookup succeeded, the fallback interval, how often the serving cell is checked, and which movements start a lookup |

Estimate and Radio only appear for positions BeaconDB looked up, not for configured or pinned ones.
