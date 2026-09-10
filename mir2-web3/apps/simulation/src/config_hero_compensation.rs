//! Mirror compensation keeps every published Hero identity retired or attached
//! at a fresh revision. It never rewinds the allocator to reuse a published ID.
use super::*;
use super::super::{AccountStore,AccountStoreMutationPlan,AccountStoreRepositorySave};

pub(in crate::config) fn prepare(
    original:&AccountStore,
    committed:&AccountStore,
    forward:&AccountStoreMutationPlan,
    rollback:&AccountStoreMutationPlan,
    receipt:&AccountStoreRepositorySave,
)->Result<(AccountStore,AccountStoreMutationPlan),String>{
    let mut restored=original.clone();
    let mut plan=rollback.clone();
    plan.fence_compensation(receipt,committed);
    for (&id,mutation) in &forward.heroes{
        let expected=receipt.heroes.heroes.get(&id).copied().ok_or("Hero commit omitted its durable row receipt")?;
        let mut record=original.shared_heroes.get(&id).cloned().unwrap_or_else(||{
            let mut value=mutation.desired.clone();value.custody=SharedHeroCustody::Released;value
        });
        record.revision=mutation.desired.revision.checked_add(1).ok_or("Hero compensation revision exhausted")?;
        if let SharedHeroCustody::Attached{attachment_revision,..}=&mut record.custody{*attachment_revision=record.revision;}
        restored.shared_heroes.insert(id,record.clone());
        plan.heroes.insert(id,HeroMutation{expected_version:Some(expected),desired:record});
    }
    restored.hero_id_high_watermark=original.hero_id_high_watermark.max(committed.hero_id_high_watermark);
    if forward.hero_allocator.is_some(){
        let expected=receipt.heroes.allocator.ok_or("Hero commit omitted its durable allocator receipt")?;
        plan.hero_allocator=Some(HeroAllocatorMutation{expected_version:Some(expected),expected_high_watermark:committed.hero_id_high_watermark,desired_high_watermark:restored.hero_id_high_watermark});
    }
    validate_complete_state(&restored)?;
    Ok((restored,plan))
}
