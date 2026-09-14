use crate::wallframe::ipc::proto::PointerMotion;
use crate::wallframe::scheduler::DisplayId;

/// A renderer can be shared by several outputs. A late leave from the old
/// output must not clear a pointer that has already entered another output.
#[derive(Default)]
pub(super) struct Focus {
    display: Option<DisplayId>,
    timestamp_us: u64,
}

impl Focus {
    pub(super) fn update(&mut self, display: DisplayId, event: &PointerMotion) -> bool {
        if event.timestamp_us != 0 && event.timestamp_us < self.timestamp_us {
            return false;
        }
        self.timestamp_us = self.timestamp_us.max(event.timestamp_us);
        if event.x < 0.0 || event.y < 0.0 {
            if self.display != Some(display) {
                return false;
            }
            self.display = None;
        } else {
            self.display = Some(display);
        }
        true
    }
}
