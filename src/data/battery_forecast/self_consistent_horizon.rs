use std::time::Duration;

const HORIZON_REFINEMENTS: usize = 8;

pub(super) fn self_consistent_forecast<Forecast>(
    first_horizon: Duration,
    forecast_over: impl Fn(Duration) -> Option<Forecast>,
    horizon_implied_by: impl Fn(&Forecast) -> Option<Duration>,
) -> Option<Forecast> {
    (0..HORIZON_REFINEMENTS).try_fold(forecast_over(first_horizon)?, |forecast, _| {
        forecast_over(horizon_implied_by(&forecast)?)
    })
}
