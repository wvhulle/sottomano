use std::{
    io::Cursor,
    mem::size_of,
    time::{Duration, Instant},
};

use pipewire::{
    core::CoreRc,
    keys,
    properties::properties,
    spa::{
        buffer::Data,
        param::{
            ParamType,
            audio::{AudioFormat, AudioInfoRaw},
        },
        pod::{
            Object, Pod, Value,
            serialize::{GenError, PodSerializer},
        },
        utils::{Direction as Flow, SpaTypes},
    },
    stream::{Stream, StreamFlags, StreamListener, StreamRc, StreamState},
};
use tokio::sync::watch;

use super::Direction;

const PUBLISH_INTERVAL: Duration = Duration::from_millis(66);
const RELEASE: Duration = Duration::from_millis(300);
const STEPS: f32 = 24.0;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Peaks {
    pub output: f32,
    pub input: f32,
}

fn channel(peaks: &mut Peaks, direction: Direction) -> &mut f32 {
    match direction {
        Direction::Output => &mut peaks.output,
        Direction::Input => &mut peaks.input,
    }
}

fn stream_name(direction: Direction) -> &'static str {
    match direction {
        Direction::Output => "sottomano-output-meter",
        Direction::Input => "sottomano-input-meter",
    }
}

fn remedy(direction: Direction) -> &'static str {
    match direction {
        Direction::Output => {
            " — the default sink exposes no monitor ports; check the WirePlumber setting \
             node.features.audio.monitor-ports"
        }
        Direction::Input => "",
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MeterError {
    #[error("encoding the capture format failed: {0}")]
    Format(GenError),
    #[error("the encoded capture format is not a valid pod")]
    UnreadableFormat,
    #[error("creating the capture stream failed: {0}")]
    Stream(#[from] pipewire::Error),
}

pub struct Meter {
    _stream: StreamRc,
    _listener: StreamListener<Capture>,
}

pub fn attach(core: &CoreRc, peaks: &watch::Sender<Peaks>) -> Vec<Meter> {
    [Direction::Output, Direction::Input]
        .into_iter()
        .filter_map(|direction| {
            capture(core, direction, peaks)
                .inspect_err(|e| {
                    tracing::debug!("audio: no {} level meter: {e}", stream_name(direction));
                })
                .ok()
        })
        .collect()
}

struct Capture {
    direction: Direction,
    envelope: Envelope,
    published_at: Instant,
    reported: bool,
    peaks: watch::Sender<Peaks>,
}

impl Capture {
    fn new(direction: Direction, peaks: watch::Sender<Peaks>) -> Self {
        Self {
            direction,
            envelope: Envelope::silent(),
            published_at: Instant::now(),
            reported: false,
            peaks,
        }
    }

    fn observe(&mut self, peak: f32, now: Instant) {
        self.envelope = self.envelope.decayed_to(now).risen_to(perceptual(peak));
        self.publish(now);
    }

    fn silence(&mut self, now: Instant) {
        self.envelope = Envelope::silent();
        self.publish(now);
    }

    #[allow(clippy::float_cmp)]
    fn publish(&mut self, now: Instant) {
        let next = quantize(self.envelope.value);
        let direction = self.direction;
        let due = now.duration_since(self.published_at) >= PUBLISH_INTERVAL;
        let published = self.peaks.send_if_modified(|peaks| {
            let current = channel(peaks, direction);
            let sustained = next != 0.0 && *current != 0.0;
            if *current == next || (sustained && !due) {
                return false;
            }
            *current = next;
            true
        });
        if published {
            self.published_at = now;
        }
    }

    fn report(&mut self, state: &StreamState) {
        match state {
            StreamState::Error(message) if !self.reported => {
                self.reported = true;
                tracing::warn!(
                    "audio: {} stopped: {message}{}",
                    stream_name(self.direction),
                    remedy(self.direction)
                );
            }
            StreamState::Streaming => self.reported = false,
            _ => {}
        }
    }
}

fn capture(
    core: &CoreRc,
    direction: Direction,
    peaks: &watch::Sender<Peaks>,
) -> Result<Meter, MeterError> {
    let mut props = properties! {
        *keys::MEDIA_TYPE => "Audio",
        *keys::MEDIA_CATEGORY => "Capture",
        *keys::NODE_PASSIVE => "true",
    };
    if direction == Direction::Output {
        props.insert(*keys::STREAM_CAPTURE_SINK, "true");
    }

    let stream = StreamRc::new(core.clone(), stream_name(direction), props)?;
    let listener = stream
        .add_local_listener_with_user_data(Capture::new(direction, peaks.clone()))
        .state_changed(|_, capture, _, state| {
            capture.report(&state);
            if !matches!(state, StreamState::Streaming) {
                capture.silence(Instant::now());
            }
        })
        .process(|stream, capture| {
            if let Some(peak) = quantum_peak(stream) {
                capture.observe(peak, Instant::now());
            }
        })
        .register()?;

    let format = enum_format()?;
    let pod = Pod::from_bytes(&format).ok_or(MeterError::UnreadableFormat)?;
    stream.connect(
        Flow::Input,
        None,
        StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
        &mut [pod],
    )?;
    Ok(Meter {
        _stream: stream,
        _listener: listener,
    })
}

fn quantum_peak(stream: &Stream) -> Option<f32> {
    let mut buffer = stream.dequeue_buffer()?;
    Some(buffer.datas_mut().first_mut().map_or(0.0, sample_peak))
}

fn sample_peak(data: &mut Data) -> f32 {
    let chunk = data.chunk();
    let offset = chunk.offset() as usize;
    let end = offset.saturating_add(chunk.size() as usize);
    data.data()
        .and_then(|samples| samples.get(offset..end.min(samples.len())))
        .map_or(0.0, |region| {
            region
                .as_chunks::<{ size_of::<f32>() }>()
                .0
                .iter()
                .map(|bytes| f32::from_le_bytes(*bytes).abs())
                .fold(0.0, f32::max)
        })
}

fn enum_format() -> Result<Vec<u8>, MeterError> {
    let mut info = AudioInfoRaw::new();
    info.set_format(AudioFormat::F32LE);
    let object = Object {
        type_: SpaTypes::ObjectParamFormat.as_raw(),
        id: ParamType::EnumFormat.as_raw(),
        properties: info.into(),
    };
    PodSerializer::serialize(Cursor::new(Vec::new()), &Value::Object(object))
        .map(|(cursor, _)| cursor.into_inner())
        .map_err(MeterError::Format)
}

#[derive(Debug, Clone, Copy)]
struct Envelope {
    value: f32,
    observed: Instant,
}

impl Envelope {
    fn silent() -> Self {
        Self {
            value: 0.0,
            observed: Instant::now(),
        }
    }

    fn decayed_to(self, now: Instant) -> Self {
        let elapsed = now.duration_since(self.observed).as_secs_f32();
        Self {
            value: self.value * (-elapsed / RELEASE.as_secs_f32()).exp(),
            observed: now,
        }
    }

    fn risen_to(self, peak: f32) -> Self {
        Self {
            value: self.value.max(peak),
            ..self
        }
    }
}

fn perceptual(amplitude: f32) -> f32 {
    amplitude.clamp(0.0, 1.0).cbrt()
}

fn quantize(value: f32) -> f32 {
    let steps = (value.clamp(0.0, 1.0) * STEPS).round();
    if steps <= 0.0 { 0.0 } else { steps / STEPS }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn capture_of(direction: Direction) -> (Capture, watch::Receiver<Peaks>) {
        let (peaks, receiver) = watch::channel(Peaks::default());
        (Capture::new(direction, peaks), receiver)
    }

    #[test]
    fn attack_is_immediate_and_release_is_gradual() {
        let (mut capture, _receiver) = capture_of(Direction::Output);
        let start = Instant::now();
        capture.observe(1.0, start);
        assert_eq!(capture.envelope.value, 1.0);

        capture.observe(0.0, start + RELEASE);
        assert!(capture.envelope.value > 0.2);
        assert!(capture.envelope.value < 0.5);

        capture.observe(0.0, start + RELEASE * 20);
        assert_eq!(quantize(capture.envelope.value), 0.0);
    }

    #[test]
    fn silence_publishes_immediately_after_a_peak() {
        let (mut capture, receiver) = capture_of(Direction::Output);
        let start = Instant::now();
        capture.observe(1.0, start);
        assert_eq!(receiver.borrow().output, 1.0);

        capture.silence(start);
        assert_eq!(*receiver.borrow(), Peaks::default());
    }

    #[test]
    fn rapid_peaks_are_rate_limited() {
        let (mut capture, receiver) = capture_of(Direction::Output);
        let start = Instant::now();
        capture.observe(1.0, start);
        capture.observe(0.001, start + Duration::from_millis(1));
        assert_eq!(receiver.borrow().output, 1.0);

        capture.observe(0.001, start + PUBLISH_INTERVAL);
        assert!(receiver.borrow().output < 1.0);
    }

    #[test]
    fn each_direction_publishes_into_its_own_channel() {
        let (peaks, receiver) = watch::channel(Peaks::default());
        let mut output = Capture::new(Direction::Output, peaks.clone());
        let mut input = Capture::new(Direction::Input, peaks);
        let start = Instant::now();

        input.observe(1.0, start);
        assert_eq!(receiver.borrow().input, 1.0);
        assert_eq!(receiver.borrow().output, 0.0);

        output.observe(1.0, start);
        assert_eq!(receiver.borrow().output, 1.0);
        assert_eq!(receiver.borrow().input, 1.0);
    }
}
