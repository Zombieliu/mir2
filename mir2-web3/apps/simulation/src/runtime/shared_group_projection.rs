//! Own-session mirror of the Gateway's transient typed online party authority.
use super::resources::{is_in_world, SessionResource, Stage5SystemsResource};
use super::session::SimulationSession;
use std::collections::BTreeSet;

impl SimulationSession {
    pub fn shared_group_permission(&self) -> Option<bool> {
        is_in_world(self.app.world()).then(|| {
            self.app
                .world()
                .resource::<Stage5SystemsResource>()
                .stage5_systems
                .group
                .allow_group
        })
    }

    /// Server-only projection. The Gateway resolves these names from exact
    /// admitted online identities; this never accepts a client roster.
    pub fn apply_shared_group_projection(
        &mut self,
        allow_group: bool,
        members: &[String],
    ) -> Result<(), String> {
        if !is_in_world(self.app.world()) {
            return Err("shared party projection has no active world character".into());
        }
        let character = self
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character
            .as_ref()
            .ok_or("shared party projection character missing")?;
        let names = members
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        if members.len() > 15
            || members.len() == 1
            || names.len() != members.len()
            || members.iter().any(|name| name.trim().is_empty())
            || (!members.is_empty()
                && !members
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(&character.name)))
        {
            return Err("shared party projection is not an admitted roster".into());
        }
        let group = &mut self
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .group;
        // AllowGroup is a saved character preference; GroupMembers are live
        // references in Crystal and are cleared by cold join/leave/teardown.
        group.allow_group = allow_group;
        group.members = members.to_vec();
        Ok(())
    }
}
