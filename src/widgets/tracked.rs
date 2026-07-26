//! A value read from a poll but written by the user, tracking both the handle
//! (where the user is dragging) and the value last *confirmed* on disk / in the
//! OS (the ghost). It shows the user's change immediately, freezes incoming
//! readings during a drag, and paces writes by confirmation rather than by a
//! timer: one write is in flight at a time, and the next fires only when the
//! previous is acknowledged. Pure state — no rendering.

/// A write the caller must perform and whose outcome it must report back via
/// [`Tracked::confirmed`]. The `must_use` is the contract: dropping it silently
/// would strand the value, since only a confirmation advances the ghost.
#[must_use = "perform the write and report the result via `confirmed`, or the value never advances"]
pub struct Write(pub f32);

#[derive(Default)]
pub struct Tracked {
    /// Optimistic display value (the handle), overriding the reading until
    /// reconciled.
    pending: Option<f32>,
    /// Between drag start and release: freezes incoming readings so a stray poll
    /// can't yank the thumb.
    dragging: bool,
    /// The value last confirmed written — drawn as the ghost, and the baseline a
    /// redundant write is dropped against.
    acked: f32,
    /// A write is in flight; the next target waits for its confirmation. This is
    /// the whole coalescing gate — there is no time throttle.
    in_flight: bool,
}

impl Tracked {
    /// What to display: the optimistic handle while interacting, else `reading`.
    pub fn value(&self, reading: f32) -> f32 {
        self.pending.unwrap_or(reading)
    }

    /// The ghost marker — the last confirmed value — shown only while the user is
    /// interacting (there is nothing to trail when the handle sits on the reading).
    pub fn ghost(&self) -> Option<f32> {
        self.pending.map(|_| self.acked)
    }

    /// Reconcile with an authoritative reading (poll / load). Ignored mid-drag so
    /// a stray poll can't yank the thumb; otherwise trust the reading and adopt it
    /// as both the display and the confirmed baseline.
    pub fn sync(&mut self, reading: f32) {
        if self.dragging {
            return;
        }
        self.pending = None;
        self.acked = reading;
    }

    /// Begin/continue a drag at `pos`; issue a write if the gate is open.
    pub fn drag(&mut self, pos: f32) -> Option<Write> {
        self.dragging = true;
        self.pending = Some(pos);
        self.issue(pos)
    }

    /// A discrete scroll to `target` (no ongoing gesture, so readings are not
    /// frozen); issue a write if the gate is open.
    pub fn scroll(&mut self, target: f32) -> Option<Write> {
        self.pending = Some(target);
        self.issue(target)
    }

    /// End the drag; re-issue the resting value so the final position still lands
    /// if the gate was closed when the last delta arrived.
    pub fn end_drag(&mut self) -> Option<Write> {
        self.dragging = false;
        self.pending.and_then(|pos| self.issue(pos))
    }

    /// Record a write's outcome. On success advance the ghost; then, if the finger
    /// has moved on, issue the next (coalesced to the latest position) write.
    // Exact `f32` comparison is intentional: `pending` and `acked` are quantized
    // slider positions, bit-identical when they name the same target.
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

    /// The gate: emit a write for `pos` unless one is already in flight or `pos`
    /// is already the confirmed value.
    // Exact `f32` comparison is intentional (see `confirmed`).
    #[allow(clippy::float_cmp)]
    fn issue(&mut self, pos: f32) -> Option<Write> {
        if self.in_flight || pos == self.acked {
            return None;
        }
        self.in_flight = true;
        Some(Write(pos))
    }
}
