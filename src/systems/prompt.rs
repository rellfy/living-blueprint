use crate::State;

const FADE_RATE: f64 = 10.0;

pub fn update_prompt(state: &mut State) {
    let prompt = &mut state.prompt;
    let target = match prompt.get_is_empty() {
        true => 0.0,
        false => 1.0,
    };
    let blend = 1.0 - (-FADE_RATE * state.delta_time).exp();
    prompt.visibility += (target - prompt.visibility) * blend;
    prompt.caret_phase += state.delta_time;
}
