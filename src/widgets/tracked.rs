use std::mem;

#[must_use = "perform the write and report the result via `confirmed` or `adjusted`, or the value \
              never advances"]
pub enum Write {
    Absolute(f32),
    Relative(f32),
}

#[derive(Debug, Clone, Copy)]
pub struct Nudge {
    pub delta: f32,
    pub optimistic: f32,
}

#[derive(Default)]
pub struct Tracked {
    pending: Option<f32>,
    queued_delta: f32,
    dragging: bool,
    acked: f32,
    reading: f32,
    in_flight: bool,
}

impl Tracked {
    pub fn value(&self) -> f32 {
        self.pending.unwrap_or(self.reading)
    }

    pub fn ghost(&self, tolerance: f32) -> Option<f32> {
        let target = self.pending?;
        ((self.reading - target).abs() > tolerance).then_some(self.reading)
    }

    pub fn sync(&mut self, reading: f32, tolerance: f32) {
        self.reading = reading;
        if self.dragging || self.in_flight || self.queued_delta != 0.0 {
            return;
        }
        if (reading - self.acked).abs() <= tolerance {
            self.pending = None;
        } else if self.pending.is_none() {
            self.acked = reading;
        }
    }

    pub fn drag(&mut self, pos: f32) -> Option<Write> {
        self.dragging = true;
        self.pending = Some(pos);
        self.next_write()
    }

    pub fn scroll(&mut self, target: f32) -> Option<Write> {
        self.pending = Some(target);
        self.next_write()
    }

    pub fn adjust(&mut self, nudge: Nudge) -> Option<Write> {
        let Nudge { delta, optimistic } = nudge;
        self.pending = Some(optimistic);
        self.queued_delta += delta;
        self.next_write()
    }

    pub fn end_drag(&mut self) -> Option<Write> {
        self.dragging = false;
        self.next_write()
    }

    pub fn confirmed(&mut self, value: f32, ok: bool) -> Option<Write> {
        self.in_flight = false;
        if ok {
            self.acked = value;
        }
        self.next_write()
    }

    pub fn adjusted(&mut self, resolved: Option<f32>) -> Option<Write> {
        self.in_flight = false;
        if let Some(value) = resolved {
            self.acked = value;
            self.reading = value;
            self.pending = (self.queued_delta != 0.0).then_some(value + self.queued_delta);
        } else {
            self.queued_delta = 0.0;
            self.pending = None;
        }
        self.next_write()
    }

    #[allow(clippy::float_cmp)]
    fn next_write(&mut self) -> Option<Write> {
        if self.in_flight {
            return None;
        }
        if self.queued_delta != 0.0 {
            self.in_flight = true;
            return Some(Write::Relative(mem::take(&mut self.queued_delta)));
        }
        match self.pending {
            Some(pos) if pos != self.acked => {
                self.in_flight = true;
                Some(Write::Absolute(pos))
            }
            _ => None,
        }
    }
}
