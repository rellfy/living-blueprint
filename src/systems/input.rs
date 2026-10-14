use crate::State;
use crate::resources::pointer::PointerRelease;
use crate::systems::agent::submit_prompt;
use vello::kurbo::{Point, Size};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const ZOOM_PER_LINE: f64 = 1.18;
const ZOOM_PER_PIXEL: f64 = 1.004;

pub fn handle_cursor_moved(state: &mut State, position: Point) {
    let Some(delta) = state.pointer.move_to(position) else {
        return;
    };
    state.camera.pan_by_screen(delta);
}

pub fn handle_mouse_button(state: &mut State, button: MouseButton, element_state: ElementState) {
    if button != MouseButton::Left {
        return;
    }
    match element_state {
        ElementState::Pressed => state.pointer.press(),
        ElementState::Released => handle_release(state),
    }
}

pub fn handle_mouse_wheel(state: &mut State, delta: MouseScrollDelta) {
    let factor = match delta {
        MouseScrollDelta::LineDelta(_, lines) => ZOOM_PER_LINE.powf(lines as f64),
        MouseScrollDelta::PixelDelta(pixels) => ZOOM_PER_PIXEL.powf(pixels.y),
    };
    let viewport = viewport_size(state);
    state
        .camera
        .zoom_at(state.pointer.position, factor, viewport);
}

pub fn handle_key(state: &mut State, event: KeyEvent) {
    if event.state != ElementState::Pressed {
        return;
    }
    match event.logical_key {
        Key::Named(NamedKey::Enter) => submit_prompt(state),
        Key::Named(NamedKey::Backspace) => state.prompt.backspace(),
        Key::Named(NamedKey::Escape) => state.prompt.clear(),
        Key::Named(NamedKey::Home) => fit_repository(state),
        _ => insert_text(state, event.text.as_deref()),
    }
}

pub fn update_hover(state: &mut State) {
    let viewport = viewport_size(state);
    let world_point = state
        .camera
        .screen_to_world(state.pointer.position, viewport);
    state.hovered = match state.pointer.get_is_dragging() {
        true => state.hovered,
        false => state
            .repository
            .as_ref()
            .and_then(|repository| repository.hit_test(world_point)),
    };
}

pub fn fit_repository(state: &mut State) {
    let viewport = viewport_size(state);
    let Some(repository) = &state.repository else {
        return;
    };
    state.camera.fly_to(repository.bounds(), viewport);
}

pub fn viewport_size(state: &State) -> Size {
    let config = &state.wgpu_resource.surface_config;
    Size::new(config.width as f64, config.height as f64)
}

fn insert_text(state: &mut State, text: Option<&str>) {
    let is_shortcut = state
        .modifiers
        .intersects(ModifiersState::CONTROL | ModifiersState::SUPER | ModifiersState::ALT);
    let Some(text) = text else {
        return;
    };
    if is_shortcut {
        return;
    }
    state.prompt.insert(text);
}

fn handle_release(state: &mut State) {
    match state.pointer.release() {
        PointerRelease::Click(position) => focus_at(state, position),
        PointerRelease::Drag(velocity) => state.camera.release_with_velocity(velocity),
        PointerRelease::None => {}
    }
}

fn focus_at(state: &mut State, screen_point: Point) {
    let viewport = viewport_size(state);
    let world_point = state.camera.screen_to_world(screen_point, viewport);
    let Some(repository) = &state.repository else {
        return;
    };
    let Some(id) = repository.hit_test(world_point) else {
        return;
    };
    state.camera.fly_to(repository.node(id).rect, viewport);
}
