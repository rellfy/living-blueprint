use crate::State;
use crate::resources::pointer::PointerRelease;
use crate::resources::prompt::Motion;
use crate::systems::agent::submit_prompt;
use crate::systems::semantic_zoom::zoom_semantically;
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
    match (button, element_state) {
        (MouseButton::Left, ElementState::Pressed) => state.pointer.press(),
        (MouseButton::Left, ElementState::Released) => handle_release(state),
        (MouseButton::Right, ElementState::Pressed) => zoom_semantically(state, false),
        _ => {}
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
    let is_command = state
        .modifiers
        .intersects(ModifiersState::CONTROL | ModifiersState::SUPER);
    match event.logical_key {
        Key::Named(NamedKey::Enter) => handle_enter(state),
        Key::Named(NamedKey::Escape) => escape_prompt(state),
        Key::Named(NamedKey::Home) if state.prompt.get_is_empty() => fit_repository(state),
        Key::Named(key) => handle_named_key(state, key, event.text.as_deref()),
        Key::Character(ref key) if is_command => handle_shortcut(state, key),
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

fn handle_enter(state: &mut State) {
    let is_shifted = state.modifiers.shift_key();
    match (is_shifted, state.prompt.get_is_caret_after_backslash()) {
        (true, _) => state.prompt.break_line(),
        (false, true) => state.prompt.break_escaped_line(),
        (false, false) => submit_prompt(state),
    }
}

fn handle_named_key(state: &mut State, key: NamedKey, text: Option<&str>) {
    let modifiers = state.modifiers;
    let is_extending = modifiers.shift_key();
    let prompt = &mut state.prompt;
    match key {
        NamedKey::ArrowLeft => prompt.move_caret(left_motion(modifiers), is_extending),
        NamedKey::ArrowRight => prompt.move_caret(right_motion(modifiers), is_extending),
        NamedKey::ArrowUp => prompt.move_caret(Motion::LineUp, is_extending),
        NamedKey::ArrowDown => prompt.move_caret(Motion::LineDown, is_extending),
        NamedKey::Home => prompt.move_caret(Motion::Start, is_extending),
        NamedKey::End => prompt.move_caret(Motion::End, is_extending),
        NamedKey::Backspace => prompt.delete(left_motion(modifiers)),
        NamedKey::Delete => prompt.delete(right_motion(modifiers)),
        _ => insert_text(state, text),
    }
}

fn handle_shortcut(state: &mut State, key: &str) {
    let is_shifted = state.modifiers.shift_key();
    match (key.to_lowercase().as_str(), is_shifted) {
        ("a", _) => state.prompt.select_all(),
        ("c", _) => copy_selection(state),
        ("x", _) => cut_selection(state),
        ("v", _) => paste_clipboard(state),
        ("z", false) => state.prompt.undo(),
        ("z", true) | ("y", _) => state.prompt.redo(),
        _ => {}
    }
}

fn escape_prompt(state: &mut State) {
    match state.prompt.get_has_selection() {
        true => state.prompt.collapse_selection(),
        false => state.prompt.clear(),
    }
}

fn copy_selection(state: &mut State) {
    if !state.prompt.get_has_selection() {
        return;
    }
    state.clipboard.write(state.prompt.selected_text());
}

fn cut_selection(state: &mut State) {
    copy_selection(state);
    state.prompt.delete_selection();
}

fn paste_clipboard(state: &mut State) {
    let Some(text) = state.clipboard.read() else {
        return;
    };
    state.prompt.paste(&text);
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

fn left_motion(modifiers: ModifiersState) -> Motion {
    match (
        modifiers.super_key(),
        modifiers.control_key() || modifiers.alt_key(),
    ) {
        (true, _) => Motion::Start,
        (false, true) => Motion::WordLeft,
        (false, false) => Motion::CharLeft,
    }
}

fn right_motion(modifiers: ModifiersState) -> Motion {
    match (
        modifiers.super_key(),
        modifiers.control_key() || modifiers.alt_key(),
    ) {
        (true, _) => Motion::End,
        (false, true) => Motion::WordRight,
        (false, false) => Motion::CharRight,
    }
}

fn handle_release(state: &mut State) {
    match state.pointer.release() {
        PointerRelease::Click => zoom_semantically(state, true),
        PointerRelease::Drag(velocity) => state.camera.release_with_velocity(velocity),
        PointerRelease::None => {}
    }
}
