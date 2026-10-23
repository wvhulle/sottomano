use chrono::{DateTime, Locale, TimeDelta, Utc};
use iced::alignment::Horizontal;
use sottomano_sources::weather::open_meteo_precipitation_model::{
    ModelQuarterHour, PrecipitationModel,
};

use crate::{
    clock::Clock,
    widget_groups::weather_group::rain_intensity::intensity_text,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        tooltip_body::{
            Cell, ColumnHeading, KeyValue, Section, TableRecord, TableRow, TooltipBody,
        },
        weather_icons::WeatherIcon,
    },
};

const MODEL_QUARTER_HOUR: TimeDelta = TimeDelta::minutes(15);

const MODEL_SCHEDULE_HEADINGS: [ColumnHeading; 3] = [
    ColumnHeading::new("Rain"),
    ColumnHeading::new(""),
    ColumnHeading::new("Type").align_x(Horizontal::Left),
];

impl TableRow<3> for &ModelQuarterHour {
    fn table_record(self) -> TableRecord<3> {
        TableRecord::new(
            Cell::Time {
                at: self.starts_at,
                format: LocalizedTimeFormat::HourAndMinute,
            },
            [
                Some(intensity_text(self.intensity).into()),
                Some(Cell::Glyph(
                    WeatherIcon::Sky(self.condition, self.daylight).into(),
                )),
                Some(self.condition.to_string().into()),
            ],
        )
    }
}

fn quarters_not_yet_over(
    model: &PrecipitationModel,
    now: DateTime<Utc>,
) -> impl Iterator<Item = &ModelQuarterHour> {
    model
        .quarters
        .iter()
        .filter(move |quarter| quarter.starts_at + MODEL_QUARTER_HOUR > now)
}

impl BarWidget for PrecipitationModel {
    fn name(&self) -> &'static str {
        "Precipitation type"
    }

    fn icon(&self) -> Glyph {
        WeatherIcon::Sky(self.current.condition, self.current.daylight).into()
    }

    fn fallback_icon() -> Glyph {
        WeatherIcon::Sprinkle.into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.current.condition.to_string())
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        TooltipBody::sections([
            Section::dictionary([
                KeyValue::new("Now", self.current.condition.to_string()),
                KeyValue::new("Rain", intensity_text(self.current.intensity)),
            ])
            .titled("Open-Meteo model"),
            Section::column_table(
                MODEL_SCHEDULE_HEADINGS,
                quarters_not_yet_over(self, clock.utc()),
            )
            .titled("Schedule"),
        ])
    }
}

#[cfg(test)]
mod tests {
    use sottomano_sources::weather::{
        buienradar_rain::precipitation::millimeter_per_hour,
        open_meteo_temperature::{Daylight, Intensity, WeatherCondition},
    };
    use uom::si::f64::Velocity;

    use super::*;
    use crate::widgets::tooltip_body::RecordCells;

    fn quarter(starts_at: DateTime<Utc>, condition: WeatherCondition) -> ModelQuarterHour {
        ModelQuarterHour {
            starts_at,
            intensity: Velocity::new::<millimeter_per_hour>(5.6),
            condition,
            daylight: Daylight::Day,
        }
    }

    #[test]
    fn model_rows_name_the_precipitation_type() {
        let row = RecordCells::from(
            (&quarter(
                Clock::real().utc(),
                WeatherCondition::Drizzle(Intensity::Light),
            ))
                .table_record(),
        );
        assert_eq!(row.cell(0), Some(&Cell::from("5.6 mm/h")));
        assert_eq!(row.cell(2), Some(&Cell::from("Light drizzle")));
    }

    #[test]
    fn quarters_already_over_leave_the_schedule() {
        let now = Clock::real().utc();
        let model = PrecipitationModel {
            current: quarter(now, WeatherCondition::Fog),
            quarters: vec![
                quarter(now - MODEL_QUARTER_HOUR, WeatherCondition::Fog),
                quarter(now, WeatherCondition::Rain(Intensity::Moderate)),
            ],
        };
        let remaining: Vec<_> = quarters_not_yet_over(&model, now).collect();
        assert_eq!(remaining, [&model.quarters[1]]);
    }

    #[test]
    fn the_bar_label_names_the_current_type() {
        let clock = Clock::real();
        let model = PrecipitationModel {
            current: quarter(clock.utc(), WeatherCondition::Drizzle(Intensity::Light)),
            quarters: Vec::new(),
        };
        assert_eq!(
            model.label(clock, Locale::default()),
            Some("Light drizzle".to_owned())
        );
    }
}
