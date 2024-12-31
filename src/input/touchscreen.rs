use std::collections::VecDeque;
use std::error::Error;
use crate::{ABSOLUTE_X_POS, ABSOLUTE_Y_POS, EV_KEY, EV_SYN, TOUCHES_BEGAN, TOUCHES_ENDED};
use crate::input::device::{InputEvent, ReadInputStream};

/// Represents a touchscreen interface.
#[derive(Default)]
pub struct Touchscreen {
    /// A ring buffer for the current touch's points. Acts as a stack where the first position is
    /// the most recently touched. Using a `VecDeque` lets us iterate freely in both directions and
    /// is space efficient.
    trail: VecDeque<(usize, usize)>,
    /// Since each event only comes with one part of the coordinate pair, we need to buffer one
    /// until the other is received. When both values are `Some`, we push to the trail and reset
    /// these values to None.
    next_x: Option<usize>,
    /// See: `next_x`.
    next_y: Option<usize>,
    /// The delta holds the coordinate vector difference from the origin of the last touch trail.
    /// It should technically be the sum of the entire `trail`. This field in particular is useful
    /// for implementing touch-and-drag interfaces where the element is offset by the touch offset.
    delta: (usize, usize),
    /// Indicates to the user that the user has lifted their finger, and that they should stop
    /// dragging, process a tap, or stop connecting lines.
    touches_ended: bool,
}

impl ReadInputStream for Touchscreen {
    fn read_events(&mut self, stream: impl Iterator<Item = InputEvent>) -> Result<bool, Box<dyn Error>> {
        for event in stream {
            match (event.r#type, event.code) {
                (EV_SYN, _,) => {
                    if let (Some(x), Some(y)) = (self.next_x, self.next_y) {
                        self.trail.push_front((x, y));
                        self.next_x = None;
                        self.next_y = None;
                    }
                }
                (EV_KEY, ABSOLUTE_X_POS) => self.next_x = Some(event.value as usize),
                (EV_KEY, ABSOLUTE_Y_POS) => self.next_y = Some(event.value as usize),
                (EV_KEY, TOUCHES_BEGAN) => self.touches_ended = false,
                (EV_KEY, TOUCHES_ENDED) => self.touches_ended = true,
                _ => {}
            }
        }

        // We assume if this was called, we can signal the user to process events. However, we
        // really shouldn't if every event in `stream` fell into the default match case.
        Ok(true)
    }
}

/// Represents a single-touch touchscreen device.
impl Touchscreen {
    /// Returns `Some((x, y))` if the user clicked/tapped on the screen.
    fn get_tap(&mut self) -> Option<(usize, usize)> {
        match self.touches_ended {
            false => None,
            true => {
                // ensure the `trail` has not deviated by more than a few (10?) pixels
                None
            }
        }
    }

    // TODO: fn click(&self) -> Option(usize, usize) : returns Some(pt) if the user clicked there.
    // TODO: Update `trail` to return a reference to the VecDeque. Returning an owned Vec is definitely slow :D
    pub(crate) fn trail(&mut self) -> Vec<(usize, usize)> {
        let res = self.trail.iter().cloned().collect();
        self.trail.truncate(0);
        res
    }

    /// Returns whether or not the user has lifted their finger. If `poll` returns true on a this
    /// device you can safely assume that touches have begun.
    pub(crate) fn touches_ended(&self) -> bool {
        self.touches_ended
    }
}
