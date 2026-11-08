use super::*;

#[test]
fn control_states_select_the_styles_defined_by_the_design_system() {
    for (state, class) in [(ControlState::Idle, "st-idle"), (ControlState::Pending, "st-pending"),
        (ControlState::Success, "st-success"), (ControlState::Error, "st-error")] {
        assert_eq!(state.class(), class);
        assert!(include_str!("../../style.css").contains(&format!(".{class}")));
    }
}
