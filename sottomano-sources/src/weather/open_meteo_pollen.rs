use std::time::Duration;

use futures::Stream;
use serde::Deserialize;
use situo_client::GeoCoordinates;

use super::Error;
use crate::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::http_json_client,
};

#[derive(Deserialize)]
struct AirQuality {
    current: PollenReadings,
}

#[derive(Deserialize)]
pub struct PollenReadings {
    #[serde(rename = "alder_pollen")]
    alder: Option<f64>,
    #[serde(rename = "birch_pollen")]
    birch: Option<f64>,
    #[serde(rename = "grass_pollen")]
    grass: Option<f64>,
    #[serde(rename = "mugwort_pollen")]
    mugwort: Option<f64>,
    #[serde(rename = "olive_pollen")]
    olive: Option<f64>,
    #[serde(rename = "ragweed_pollen")]
    ragweed: Option<f64>,
}

impl PollenReadings {
    fn grains_per_cubic_metre(&self, allergen: Allergen) -> Option<f64> {
        match allergen {
            Allergen::Alder => self.alder,
            Allergen::Birch => self.birch,
            Allergen::Grass => self.grass,
            Allergen::Mugwort => self.mugwort,
            Allergen::Olive => self.olive,
            Allergen::Ragweed => self.ragweed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Allergen {
    Alder,
    Birch,
    Grass,
    Mugwort,
    Olive,
    Ragweed,
}

impl Allergen {
    pub const ALL: [Allergen; 6] = [
        Allergen::Alder,
        Allergen::Birch,
        Allergen::Grass,
        Allergen::Mugwort,
        Allergen::Olive,
        Allergen::Ragweed,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Allergen::Alder => "Alder",
            Allergen::Birch => "Birch",
            Allergen::Grass => "Grass",
            Allergen::Mugwort => "Mugwort",
            Allergen::Olive => "Olive",
            Allergen::Ragweed => "Ragweed",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AllergenConcentration {
    pub allergen: Allergen,
    pub grains_per_cubic_metre: f64,
}

impl AllergenConcentration {
    #[must_use]
    pub fn is_airborne(&self) -> bool {
        self.grains_per_cubic_metre > 0.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pollen {
    pub concentrations: Vec<AllergenConcentration>,
}

impl Pollen {
    fn of(readings: &PollenReadings) -> Self {
        Self {
            concentrations: Allergen::ALL
                .into_iter()
                .filter_map(|allergen| {
                    Some(AllergenConcentration {
                        allergen,
                        grains_per_cubic_metre: readings.grains_per_cubic_metre(allergen)?,
                    })
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn concentration(&self, allergen: Allergen) -> Option<&AllergenConcentration> {
        self.concentrations
            .iter()
            .find(|concentration| concentration.allergen == allergen)
    }

    pub fn airborne(&self) -> impl Iterator<Item = &AllergenConcentration> {
        self.concentrations
            .iter()
            .filter(|concentration| concentration.is_airborne())
    }
}

impl SnapshotSource for Pollen {
    type Error = Error;
    type StreamInputs = GeoCoordinates;

    const NAME: &'static str = "pollen";

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_mins(30)).poll_deliveries(move || Self::poll(inputs))
    }
}

impl Pollen {
    async fn poll(here: GeoCoordinates) -> Result<Self, Error> {
        let url = format!(
            "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&current=alder_pollen,birch_pollen,grass_pollen,mugwort_pollen,olive_pollen,ragweed_pollen&timezone=auto",
            here.latitude_degrees(),
            here.longitude_degrees()
        );
        let air: AirQuality = http_json_client::get_json(&url, &[], 10).await?;
        Ok(Pollen::of(&air.current))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AIR_QUALITY: &str = r#"{
        "current": {
            "time": "2026-08-27T10:00", "interval": 3600,
            "alder_pollen": 0.0, "birch_pollen": 0.0, "grass_pollen": 0.3,
            "mugwort_pollen": 0.7, "olive_pollen": 0.0, "ragweed_pollen": 0.7
        }
    }"#;

    const AIR_QUALITY_OUTSIDE_EUROPE: &str = r#"{
        "current": {
            "time": "2026-10-01T02:00", "interval": 3600,
            "alder_pollen": null, "birch_pollen": null, "grass_pollen": null,
            "mugwort_pollen": null, "olive_pollen": null, "ragweed_pollen": null
        }
    }"#;

    #[test]
    fn allergens_without_model_coverage_are_left_out() {
        let air: AirQuality = serde_json::from_str(AIR_QUALITY_OUTSIDE_EUROPE).unwrap();
        let pollen = Pollen::of(&air.current);
        assert!(pollen.concentrations.is_empty());
        assert_eq!(pollen.airborne().count(), 0);
    }

    #[test]
    fn allergens_select_their_own_species() {
        let air: AirQuality = serde_json::from_str(AIR_QUALITY).unwrap();
        let pollen = Pollen::of(&air.current);
        let grains = |allergen| {
            pollen
                .concentration(allergen)
                .unwrap()
                .grains_per_cubic_metre
        };
        assert!((grains(Allergen::Grass) - 0.3).abs() < f64::EPSILON);
        assert!(grains(Allergen::Birch).abs() < f64::EPSILON);
        assert_eq!(pollen.concentrations.len(), Allergen::ALL.len());
    }

    #[test]
    fn only_allergens_with_pollen_in_the_air_are_airborne() {
        let air: AirQuality = serde_json::from_str(AIR_QUALITY).unwrap();
        let airborne: Vec<Allergen> = Pollen::of(&air.current)
            .airborne()
            .map(|concentration| concentration.allergen)
            .collect();
        assert_eq!(
            airborne,
            [Allergen::Grass, Allergen::Mugwort, Allergen::Ragweed]
        );
    }
}
