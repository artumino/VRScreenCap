use std::error::Error;

use mint::Vector3;
use openxr::{FrameState, Space, Time};

pub fn get_view_acceleration_vector(reference_space: &Space, view_space: &Space, frame_state: &FrameState) -> Result<Vector3<f32>, Box<dyn Error>> {
    let origin_time = Time::from_nanos(frame_state.predicted_display_time.as_nanos() - frame_state.predicted_display_period.as_nanos());
    let (_, origin_velocity) = view_space.relate(reference_space, origin_time)?;
    let (_, predicted_velocity) = view_space.relate(reference_space, frame_state.predicted_display_time)?;

    if !predicted_velocity.velocity_flags.contains(openxr::SpaceVelocityFlags::LINEAR_VALID) ||
        !origin_velocity.velocity_flags.contains(openxr::SpaceVelocityFlags::LINEAR_VALID) {
        return Ok(Vector3 { x: 0.0, y: 0.0, z: 0.0 });
    }
    let acceleration = Vector3 {
        x: predicted_velocity.linear_velocity.x - origin_velocity.linear_velocity.x,
        y: predicted_velocity.linear_velocity.y - origin_velocity.linear_velocity.y,
        z: predicted_velocity.linear_velocity.z - origin_velocity.linear_velocity.z,
    };

    Ok(acceleration)
}