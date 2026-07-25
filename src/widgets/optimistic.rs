//! A value read from a poll but written by the user: it shows the user's change
//! immediately (optimistically), freezes incoming readings during a drag, and
//! drops a write whose value is already in effect. Pure state — no rendering —
//! so the poll↔write model can be reasoned about (and reused) on its own.

#[derive(Default)]
pub struct Optimistic {
    /// Optimistic display value, overriding the reading until reconciled.
    pending: Option<f32>,
    dragging: bool,
    /// Value believed to be in effect; a commit matching it is dropped.
    written: Option<f32>,
}

impl Optimistic {
    /// What to display: the optimistic value while interacting, else `reading`.
    pub fn value(&self, reading: f32) -> f32 {
        self.pending.unwrap_or(reading)
    }

    /// Reconcile with an authoritative reading (poll / load). Ignored mid-drag
    /// so a stray poll can't yank the thumb; otherwise trust the reading and
    /// follow it as the dedup baseline, so an external change is adopted.
    pub fn sync(&mut self, reading: f32) {
        if self.dragging {
            return;
        }
        self.pending = None;
        self.written = Some(reading);
    }

    /// Begin/continue a drag at `pos` (display only, no write).
    pub fn drag(&mut self, pos: f32) {
        self.dragging = true;
        self.pending = Some(pos);
    }

    pub fn end_drag(&mut self) {
        self.dragging = false;
    }

    /// Record a committed value; returns `Some(value)` to write, or `None` when
    /// it is already in effect.
    pub fn commit(&mut self, value: f32) -> Option<f32> {
        if self.written == Some(value) {
            return None;
        }
        self.written = Some(value);
        self.pending = Some(value);
        Some(value)
    }
}
