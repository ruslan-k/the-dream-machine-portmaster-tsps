//! Filter only the expected SDL warp echo already delivered to Ruffle.
#[derive(Default)]
pub struct WarpEchoFilter { pending: Option<(i32,i32)> }
impl WarpEchoFilter {
    pub fn mark(&mut self, x:i32, y:i32) { self.pending=Some((x,y)); }
    pub fn should_forward(&mut self, x:i32, y:i32) -> bool {
        self.pending.take() != Some((x,y))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn expected_warp_echo_is_skipped_once() {
        let mut f=WarpEchoFilter::default(); f.mark(320,240);
        assert!(!f.should_forward(320,240));
        assert!(f.should_forward(320,240));
    }
    #[test] fn real_mouse_motion_is_preserved() {
        let mut f=WarpEchoFilter::default(); f.mark(320,240);
        assert!(f.should_forward(321,240));
        assert!(f.should_forward(320,240));
    }
    #[test] fn no_warp_does_not_filter_events() {
        let mut f=WarpEchoFilter::default();
        assert!(f.should_forward(0,0)); assert!(f.should_forward(0,0));
    }
}
