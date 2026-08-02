#[must_use = "perform the write and report the result via `confirmed`, or the value never advances"]
pub struct Write(pub f32);

#[derive(Default)]
pub struct Tracked {
    pending: Option<f32>,
    dragging: bool,
    acked: f32,
    reading: f32,
    in_flight: bool,
}

impl Tracked {
    pub fn value(&self) -> f32 {
        self.pending.unwrap_or(self.reading)
    }

    pub fn ghost(&self) -> Option<f32> {
        self.pending.map(|_| self.acked)
    }

    pub fn sync(&mut self, reading: f32) {
        self.reading = reading;
        if self.dragging {
            return;
        }
        self.pending = None;
        self.acked = reading;
    }

    pub fn drag(&mut self, pos: f32) -> Option<Write> {
        self.dragging = true;
        self.pending = Some(pos);
        self.issue(pos)
    }

    pub fn scroll(&mut self, target: f32) -> Option<Write> {
        self.pending = Some(target);
        self.issue(target)
    }

    pub fn end_drag(&mut self) -> Option<Write> {
        self.dragging = false;
        self.pending.and_then(|pos| self.issue(pos))
    }

    #[allow(clippy::float_cmp)]
    pub fn confirmed(&mut self, value: f32, ok: bool) -> Option<Write> {
        self.in_flight = false;
        if ok {
            self.acked = value;
        }
        match self.pending {
            Some(pos) if pos != self.acked => self.issue(pos),
            _ => None,
        }
    }

    #[allow(clippy::float_cmp)]
    fn issue(&mut self, pos: f32) -> Option<Write> {
        if self.in_flight || pos == self.acked {
            return None;
        }
        self.in_flight = true;
        Some(Write(pos))
    }
}
