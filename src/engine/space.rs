use cgmath::Rotation3;

pub struct AppSpace {
    reference_space: openxr::Space,
    view_space: openxr::Space,
    override_space: Option<openxr::Space>,
}

impl AppSpace {
    pub fn new<G: openxr::Graphics>(session: &openxr::Session<G>) -> anyhow::Result<Self> {
        Ok(Self {
            reference_space: session.create_reference_space(
                openxr::ReferenceSpaceType::LOCAL,
                openxr::Posef::IDENTITY,
            )?,
            view_space: session.create_reference_space(
                openxr::ReferenceSpaceType::VIEW,
                openxr::Posef::IDENTITY,
            )?,
            override_space: None,
        })
    }

    pub fn space(&self) -> &openxr::Space {
        match self.override_space {
            Some(ref space) => space,
            None => &self.reference_space,
        }
    }

    pub fn view_space(&self) -> &openxr::Space {
        &self.view_space
    }

    pub fn reference_space(&self) -> &openxr::Space {
        &self.reference_space
    }

    pub fn recenter<G: openxr::Graphics>(
        &mut self,
        session: &openxr::Session<G>,
        last_predicted_frame_time: openxr::Time,
        horizon_locked: bool,
        delay: i64,
    ) -> anyhow::Result<()> {
        let mut view_location_pose = self
            .view_space
            .locate(
                &self.reference_space,
                openxr::Time::from_nanos(last_predicted_frame_time.as_nanos() - delay),
            )?
            .pose;
        let quaternion =
            cgmath::Quaternion::from(mint::Quaternion::from(view_location_pose.orientation));
        let forward = cgmath::Vector3::new(0.0, 0.0, 1.0);
        let look_dir = quaternion * forward;
        let yaw = cgmath::Rad(look_dir.x.atan2(look_dir.z));
        let clean_orientation = if horizon_locked {
            cgmath::Quaternion::from_angle_y(yaw)
        } else {
            let padj = (look_dir.x * look_dir.x + look_dir.z * look_dir.z).sqrt();
            let pitch = -cgmath::Rad(look_dir.y.atan2(padj));
            cgmath::Quaternion::from_angle_y(yaw) * cgmath::Quaternion::from_angle_x(pitch)
        };
        view_location_pose.orientation = openxr::Quaternionf {
            x: clean_orientation.v.x,
            y: clean_orientation.v.y,
            z: clean_orientation.v.z,
            w: clean_orientation.s,
        };

        self.override_space = Some(
            session
                .create_reference_space(openxr::ReferenceSpaceType::LOCAL, view_location_pose)?,
        );
        Ok(())
    }
}
