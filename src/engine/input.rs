use std::{error::Error, time::Instant};

use openxr::{Action, ActionSet, Binding, FrameState, Instance, Path, Posef, Session, Space};

pub struct InputContext {
    pub default: ActionSet,
    pub default_right_hand: Action<Posef>,
    pub default_left_hand: Action<Posef>,
    pub default_right_hand_space: Option<Space>,
    pub default_left_hand_space: Option<Space>,
    pub input_state: Option<InputState>
}

pub struct InputState {
    pub hands_near_head: u8,
    pub near_start: Instant,
    pub count_change: Instant,
}

impl InputContext {
    pub fn init(xr_instance: &Instance) -> Result<InputContext, Box<dyn Error>> {
        let default_set =
            xr_instance.create_action_set("default", "Default controller actions", 0)?;

        let right_hand = default_set.create_action("right_hand", "Right Hand Controller", &[])?;

        let left_hand = default_set.create_action("left_hand", "Left Hand Controller", &[])?;

        xr_instance.suggest_interaction_profile_bindings(
            xr_instance.string_to_path("/interaction_profiles/khr/simple_controller")?,
            &[
                Binding::new(
                    &right_hand,
                    xr_instance.string_to_path("/user/hand/right/input/grip/pose")?,
                ),
                Binding::new(
                    &left_hand,
                    xr_instance.string_to_path("/user/hand/left/input/grip/pose")?,
                ),
            ],
        )?;

        Ok(InputContext {
            default: default_set,
            default_right_hand: right_hand,
            default_left_hand: left_hand,
            default_right_hand_space: None,
            default_left_hand_space: None,
            input_state: None,
        })
    }

    pub fn attach_to_session<T>(&mut self, xr_session: &Session<T>) -> Result<(), Box<dyn Error>> {
        xr_session.attach_action_sets(&[&self.default])?;

        self.default_right_hand_space = Some(self.default_right_hand.create_space(
            xr_session.clone(),
            Path::NULL,
            Posef::IDENTITY,
        )?);

        self.default_left_hand_space = Some(self.default_left_hand.create_space(
            xr_session.clone(),
            Path::NULL,
            Posef::IDENTITY,
        )?);

        Ok(())
    }

    pub fn process_inputs<T>(
        &mut self,
        xr_session: &Session<T>,
        xr_frame_state: &FrameState,
        _xr_reference_space: &Space,
        xr_view_space: &Space,
    ) {
        xr_session.sync_actions(&[(&self.default).into()]).unwrap();

        let right_location = self
            .default_right_hand_space
            .as_ref()
            .unwrap()
            .locate(xr_view_space, xr_frame_state.predicted_display_time)
            .unwrap();

        let left_location = self
            .default_left_hand_space
            .as_ref()
            .unwrap()
            .locate(xr_view_space, xr_frame_state.predicted_display_time)
            .unwrap();

        let right_hand_distance = (right_location.pose.position.x.powi(2)
            + right_location.pose.position.y.powi(2)
            + right_location.pose.position.z.powi(2))
        .sqrt();

        let left_hand_distance = (left_location.pose.position.x.powi(2)
            + left_location.pose.position.y.powi(2)
            + left_location.pose.position.z.powi(2))
        .sqrt();

        let right_active = self.default_right_hand.is_active(xr_session, Path::NULL).unwrap();
        let left_active = self.default_left_hand.is_active(xr_session, Path::NULL).unwrap();

        let hands_near_head =
            ((right_active && right_hand_distance < 0.3) as u8) + ((left_active && left_hand_distance < 0.3) as u8);

        let near_start = if self.input_state.is_some() && self.input_state.as_ref().unwrap().hands_near_head > 0 {
            self.input_state.as_ref().unwrap().near_start
        } else {
            Instant::now()
        };

        let count_change = if self.input_state.is_none() || self.input_state.as_ref().unwrap().hands_near_head != hands_near_head {
            Instant::now()
        } else {
            self.input_state.as_ref().unwrap().count_change
        };

        self.input_state = Some(InputState {
            hands_near_head,
            near_start,
            count_change
        });
    }
}