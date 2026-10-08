//! Fresh server-owned pet XP admission, committed with its Human's source save.
//! Online lives never enter the durable PetInfo snapshot.
use bevy_ecs::prelude::{Resource, World};
use super::session::SimulationSession;
use super::zone::{ZonePetExperienceAdmission, ZoneSavedPetSnapshot};

#[derive(Debug, Clone, Default, Resource)]
pub(super) struct SharedPetProgress {
    snapshot: ZoneSavedPetSnapshot,
    admission: Option<ZonePetExperienceAdmission>,
    earned_steps: Vec<u32>,
    committed_steps: Vec<u32>,
}

pub(super) fn capture(world: &World) -> SharedPetProgress {
    world.get_resource::<SharedPetProgress>().cloned().unwrap_or_default()
}
pub(super) fn restore(world: &mut World, value: SharedPetProgress) { world.insert_resource(value); }
pub(super) fn saved_snapshot(world: &World) -> ZoneSavedPetSnapshot {
    world.get_resource::<SharedPetProgress>().map(|state| state.snapshot.clone()).unwrap_or_default()
}
pub(super) fn restore_saved_snapshot(world: &mut World, snapshot: &ZoneSavedPetSnapshot) -> Result<(),String> {
    snapshot.validate_canonical()?;
    world.insert_resource(SharedPetProgress { snapshot: snapshot.clone(), ..Default::default() });
    Ok(())
}
pub(super) fn has_earned_steps(world: &World) -> bool {
    world.get_resource::<SharedPetProgress>().is_some_and(|state| !state.earned_steps.is_empty())
}
pub(super) fn has_live_admission(world: &World) -> bool {
    world.get_resource::<SharedPetProgress>().is_some_and(|state| state.admission.as_ref().is_some_and(|admission|!admission.is_empty()))
}
pub(super) fn acknowledge(world: &mut World) {
    if let Some(mut state)=world.get_resource_mut::<SharedPetProgress>() {
        let steps=std::mem::take(&mut state.earned_steps);
        state.committed_steps.extend(steps);
    }
}
pub(super) fn record_gain_or_reject(world: &mut World, amount:u32) -> bool {
    if amount==0 || !has_live_admission(world) { return true; }
    if !super::shared_guild_experience::source_command_active(world) {
        super::shared_guild_experience::reject_source(world,"pet GainExp requires complete source checkpoint".into());
        return false;
    }
    let result=(|| {
        let mut state=world.resource_mut::<SharedPetProgress>();
        let mut steps=state.earned_steps.clone();
        steps.push(amount);
        // More than one source command may settle before the Gateway mirrors
        // the first. Admission is still the original pet state until that
        // mirror, so include its committed steps in the next durable image.
        let mut all_steps=state.committed_steps.clone();
        all_steps.extend_from_slice(&steps);
        let snapshot=state.admission.as_ref().ok_or("pet XP admission disappeared")?.saved_after_earned_steps(&all_steps)?;
        state.snapshot=snapshot;
        state.earned_steps=steps;
        Ok::<_,String>(())
    })();
    match result {
        Ok(())=>true,
        Err(error)=>{super::shared_guild_experience::reject_source(world,error);false}
    }
}

impl SimulationSession {
    /// Trusted Gateway capture only; this method is not a client command.
    pub fn set_shared_pet_experience_context(&mut self, snapshot:ZoneSavedPetSnapshot,
        admission:Option<ZonePetExperienceAdmission>)->Result<(),String> {
        snapshot.validate_canonical()?;
        if self.app.world().get_resource::<SharedPetProgress>().is_some_and(|state|
            !state.earned_steps.is_empty() || !state.committed_steps.is_empty()) {
            return Err("pet source progress must be acknowledged before replacing admission".into());
        }
        self.app.world_mut().insert_resource(SharedPetProgress{snapshot,admission,..Default::default()});
        Ok(())
    }
    pub fn take_shared_pet_experience_commit(&mut self)->Option<(ZonePetExperienceAdmission,Vec<u32>)> {
        let mut state=self.app.world_mut().get_resource_mut::<SharedPetProgress>()?;
        if state.committed_steps.is_empty(){return None;}
        let admission=state.admission.clone()?;
        let steps=std::mem::take(&mut state.committed_steps);
        Some((admission,steps))
    }
    pub fn pending_shared_pet_experience_commit(&self)->Option<(ZonePetExperienceAdmission,Vec<u32>)> {
        let state=self.app.world().get_resource::<SharedPetProgress>()?;
        if state.committed_steps.is_empty(){return None;}
        Some((state.admission.clone()?,state.committed_steps.clone()))
    }
    pub fn acknowledge_shared_pet_experience_commit(&mut self,steps:&[u32])->Result<(),String> {
        let mut state=self.app.world_mut().get_resource_mut::<SharedPetProgress>()
            .ok_or("pet progress acknowledgment lost source state")?;
        if steps.is_empty() || !state.committed_steps.starts_with(steps){
            return Err("pet progress acknowledgment does not match committed source".into());
        }
        state.committed_steps.drain(..steps.len());
        Ok(())
    }
    pub fn active_saved_pet_snapshot(&self)->ZoneSavedPetSnapshot { saved_snapshot(self.app.world()) }
}
