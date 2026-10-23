use chrono::{DateTime, TimeDelta, Utc};
use iced::alignment::Horizontal;
use sottomano_sources::weather::open_meteo_precipitation_model::{
    ModelQuarterHour, PrecipitationModel,
};

use crate::{
    widget_groups::weather_group::rain_intensity::intensity_text,
    widgets::{
        localized_time::LocalizedTimeFormat,
        tooltip_body::{Cell, ColumnHeading, KeyValue, Section, TableRecord, TableRow},
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

pub fn precipitation_model_sections(
    model: &PrecipitationModel,
    now: DateTime<Utc>,
) -> Vec<Section> {
    let current = &model.current;
    vec![
        Section::dictionary([KeyValue::new(
            "Now",
            format!(
                "{} · {}",
                intensity_text(current.intensity),
                current.condition
            ),
        )])
        .titled("Open-Meteo model"),
        Section::column_table(
            MODEL_SCHEDULE_HEADINGS,
            model
                .quarters
                .iter()
                .filter(|quarter| quarter.starts_at + MODEL_QUARTER_HOUR > now),
        )
        .titled("Model schedule"),
    ]
}

#[cfg(test)]
mod tests {
    use sottomano_sources::weather::{
        buienradar_rain::precipitation::millimeter_per_hour,
        open_meteo_temperature::{Daylight, Intensity, WeatherCondition},
    };
    use uom::si::f64::Velocity;

    use super::*;
    use crate::{
        clock::Clock,
        widgets::tooltip_body::{RecordCells, SectionBody},
    };

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
        let sections = precipitation_model_sections(&model, now);
        let SectionBody::ColumnTable { records, .. } = sections[1].body() else {
            panic!("model schedule is a column table");
        };
        assert_eq!(records.len(), 1);
    }
}
