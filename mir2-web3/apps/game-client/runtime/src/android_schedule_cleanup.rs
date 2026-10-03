//! Keep the Schedules resource available while initializing Android's renderer
//! systems. Extracting the whole resource re-created it inside resource_scope,
//! which panicked in debug builds and overwrote the replacement in release.
use bevy::{
    ecs::schedule::{IntoSystemSet, ScheduleCleanupPolicy, ScheduleError, ScheduleLabel},
    prelude::*,
};

pub(crate) fn remove_systems<M>(
    world: &mut World,
    label: impl ScheduleLabel,
    set: impl IntoSystemSet<M>,
) -> Result<usize, ScheduleError> {
    world.schedule_scope(label, |world, schedule| {
        schedule.remove_systems_in_set(set, world, ScheduleCleanupPolicy::RemoveSystemsOnly)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::schedule::Schedules;
    #[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct RenderFixture;
    #[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct OtherFixture;
    #[derive(Resource, Default)]
    struct Count(u32);
    fn removed(_schedules: Res<Schedules>, mut count: ResMut<Count>) {
        count.0 += 100;
    }
    fn retained(mut count: ResMut<Count>) {
        count.0 += 1;
    }

    fn fixture() -> World {
        let mut world = World::new();
        world.init_resource::<Count>();
        world.init_resource::<Schedules>();
        let mut schedule = Schedule::new(RenderFixture);
        schedule.add_systems((removed, retained));
        world.add_schedule(schedule);
        let mut other = Schedule::new(OtherFixture);
        other.add_systems(retained);
        world.add_schedule(other);
        world
    }
    #[test]
    fn removal_keeps_resource_and_other_schedules_alive() {
        let mut world = fixture();
        assert_eq!(
            remove_systems(&mut world, RenderFixture, removed).unwrap(),
            1
        );
        assert!(world.resource::<Schedules>().get(OtherFixture).is_some());
        world.run_schedule(RenderFixture);
        world.run_schedule(OtherFixture);
        assert_eq!(world.resource::<Count>().0, 2);
    }
    #[test]
    fn scoped_removal_does_not_require_running_the_unsupported_system() {
        let mut world = fixture();
        remove_systems(&mut world, RenderFixture, removed).unwrap();
        assert_eq!(world.resource::<Count>().0, 0);
    }
}
