use std::mem;

#[must_use = "perform the write and report the outcome via `absolute_written` or \
              `relative_written`, or the setting never advances"]
pub enum Write {
    Absolute(f32),
    Relative(f32),
}

#[derive(Debug, Clone, Copy)]
pub struct Nudge {
    pub delta: f32,
    pub requested: f32,
}

#[derive(Default)]
pub struct DeviceSetting {
    requested: Option<f32>,
    unsent_delta: f32,
    dragging: bool,
    accepted: f32,
    reported: f32,
    writing: bool,
}

impl DeviceSetting {
    pub fn wanted(&self) -> f32 {
        self.requested.unwrap_or(self.reported)
    }

    pub fn lagging(&self, step: f32) -> Option<f32> {
        let wanted = self.requested?;
        (!reached(self.reported, wanted, step)).then_some(self.reported)
    }

    pub fn observe(&mut self, reported: f32, step: f32) {
        self.reported = reported;
        if self.request_unsettled() {
            return;
        }
        if reached(reported, self.accepted, step) {
            self.requested = None;
        } else if self.requested.is_none() {
            self.accepted = reported;
        }
    }

    pub fn drag(&mut self, position: f32) -> Option<Write> {
        self.dragging = true;
        self.requested = Some(position);
        self.next_write()
    }

    pub fn scroll(&mut self, target: f32) -> Option<Write> {
        self.requested = Some(target);
        self.next_write()
    }

    pub fn adjust(&mut self, nudge: Nudge) -> Option<Write> {
        let Nudge { delta, requested } = nudge;
        self.requested = Some(requested);
        self.unsent_delta += delta;
        self.next_write()
    }

    pub fn end_drag(&mut self) -> Option<Write> {
        self.dragging = false;
        self.next_write()
    }

    pub fn absolute_written(&mut self, value: f32, ok: bool) -> Option<Write> {
        self.writing = false;
        if ok {
            self.accepted = value;
        }
        self.next_write()
    }

    pub fn relative_written(&mut self, resolved: Option<f32>) -> Option<Write> {
        self.writing = false;
        if let Some(value) = resolved {
            self.accepted = value;
            self.reported = value;
            self.requested = self.has_unsent_delta().then_some(value + self.unsent_delta);
        } else {
            self.unsent_delta = 0.0;
            self.requested = None;
        }
        self.next_write()
    }

    fn request_unsettled(&self) -> bool {
        self.dragging || self.writing || self.has_unsent_delta()
    }

    #[allow(clippy::float_cmp)]
    fn has_unsent_delta(&self) -> bool {
        self.unsent_delta != 0.0
    }

    #[allow(clippy::float_cmp)]
    fn next_write(&mut self) -> Option<Write> {
        if self.writing {
            return None;
        }
        if self.has_unsent_delta() {
            self.writing = true;
            return Some(Write::Relative(mem::take(&mut self.unsent_delta)));
        }
        match self.requested {
            Some(wanted) if wanted != self.accepted => {
                self.writing = true;
                Some(Write::Absolute(wanted))
            }
            _ => None,
        }
    }
}

fn reached(reported: f32, wanted: f32, step: f32) -> bool {
    (reported - wanted).abs() <= step / 2.0
}
