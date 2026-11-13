# Weather

Conditions where you are, from the location fix to rain and pollen.

## Weather: Rain

Rain in the coming hours on one timeline, split by hour. Each stretch of time comes from the most trusted source that covers it: radar nowcasts (Buienradar, every five minutes) rank above forecast models (Open-Meteo, every quarter-hour), and the model takes over where the radar ends or has no coverage. Within the radar's range the model only adds the precipitation type, and only where both see rain.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/weather_precipitation.webp" width="884" alt="Hovering the rain item in the weather group">

## Weather: Other readings

These open from the weather group like the rain reading above.

| Reading | What it shows |
|---|---|
| Location | How rough the location fix is and which source produced it. |
| Wind | Wind speed, gusts and direction, with the Beaufort force in the panel. |
| Pollen | Today's pollen per allergen. |
