use super::*;

#[test]
fn control_states_select_the_styles_defined_by_the_design_system() {
    let styles = [include_str!("../../styles/base.css"), include_str!("../../styles/panels.css")].concat();
    for (state, class) in [(ControlState::Idle, "st-idle"), (ControlState::Pending, "st-pending"),
        (ControlState::Success, "st-success"), (ControlState::Error, "st-error")] {
        assert_eq!(state.class(), class);
        assert!(styles.contains(&format!(".{class}")));
    }
}
