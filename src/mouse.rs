use winit::event::MouseScrollDelta;

pub trait MouseScrollDeltaExt {
    fn is_up(&self) -> bool;
}

impl MouseScrollDeltaExt for MouseScrollDelta {
    fn is_up(&self) -> bool {
        match self {
            MouseScrollDelta::LineDelta(_, y) => y.is_sign_positive(),
            MouseScrollDelta::PixelDelta(v) => v.y.is_sign_positive(),
        }
    }
}
