//! Server-only rebinding after a verified staging factory transfers its world.
use super::*;

impl HostedZoneOwnerCommandClient {
    pub(crate) fn rebind_adopted_replica_factory(
        &self,
        factory: &SharedInProcessZoneRuntimeFactory,
    ) -> Result<(), String> {
        let mut runtime = self.runtime.lock().map_err(|_| "hosted runtime poisoned")?;
        let runtime = runtime
            .as_mut()
            .ok_or("hosted runtime already handed off")?;
        let shared = runtime
            .as_any_mut()
            .downcast_mut::<SharedInProcessZoneSessionRuntime>()
            .ok_or("replica factory rebind requires shared runtime")?;
        let resources = factory
            .zones
            .lock()
            .map_err(|_| "replica factory poisoned")?
            .get(&shared.inventory_zone_id)
            .cloned()
            .ok_or("adopted Zone resource missing")?;
        if !Arc::ptr_eq(&resources.zone_state, &shared.zone_state) {
            return Err("replica factory rebind world identity mismatch".into());
        }
        let generation = factory
            .shared_mentors
            .lock()
            .map_err(|_| "social coordinator poisoned")?
            .generation
            .clone();
        shared.ranking_zones = factory.zones.clone();
        shared.ranking_replica_zones = factory.replica_zone_ids.clone();
        shared.shared_mentors = factory.shared_mentors.clone();
        shared.shared_social_generation = generation;
        shared.last_social_refresh = None;
        shared.last_experience_projection = None;
        shared.next_social_refresh_ms = 0;
        shared.next_ranking_inspect_ms = 0;
        Ok(())
    }
}
