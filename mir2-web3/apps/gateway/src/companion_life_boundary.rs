//! Host-only life scope and receive-only trade authority. No file, ledger,
//! model, balance mutation or client-supplied capability is consulted here.
use super::*;
use crate::companion_relay::{CharacterBinding, CompanionPolicy};
use crate::companion_supply::{LifeProjectionHandle, LifeScopeStatus};
use mir2_simulation::{ZoneActionBoundary, ZoneActionBoundaryRequest, ZoneActionKind};
#[path = "companion_homeward.rs"]
mod homeward;

pub(super) struct CompanionLifeBoundary {
    policy: Arc<CompanionPolicy>,
    projection: Option<LifeProjectionHandle>,
    homeward: homeward::HomewardCache,
}
impl fmt::Debug for CompanionLifeBoundary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompanionLifeBoundary")
            .finish_non_exhaustive()
    }
}
fn binding(account: &str, index: i32) -> CharacterBinding {
    CharacterBinding {
        account_id: account.into(),
        character_index: index,
    }
}
impl CompanionLifeBoundary {
    pub(super) fn pair_allowed(&self, first: &ZonePresenceKey, second: &ZonePresenceKey) -> bool {
        let first = binding(&first.account_id, first.character_index);
        let second = binding(&second.account_id, second.character_index);
        self.policy
            .actor_owner(&first)
            .is_none_or(|owner| owner == &second)
            && self
                .policy
                .actor_owner(&second)
                .is_none_or(|owner| owner == &first)
    }
    pub(super) fn offer_allowed(&self, offer: &SharedTradeOffer) -> bool {
        self.policy
            .actor_owner(&binding(&offer.account_id, offer.character_index))
            .is_none()
            || offer.gold == 0 && offer.items.is_empty()
    }
    pub(super) fn actor(&self, key: &ZonePresenceKey) -> bool {
        self.policy
            .actor_owner(&binding(&key.account_id, key.character_index))
            .is_some()
    }
}
impl ZoneActionBoundary for CompanionLifeBoundary {
    fn allows(&self, r: &ZoneActionBoundaryRequest<'_>) -> bool {
        let actor = binding(r.account_id, r.character_index);
        if self.policy.actor_owner(&actor).is_none() {
            return true;
        }
        let Some(handle) = &self.projection else {
            return true;
        };
        let Ok(projection) = handle.read() else {
            return false;
        };
        if projection.poisoned {
            return false;
        }
        let Some(scope) = projection.actors.get(&actor) else {
            return true;
        };
        let inside = |p: &Point| {
            (scope.bounds.min_x..=scope.bounds.max_x).contains(&p.x)
                && (scope.bounds.min_y..=scope.bounds.max_y).contains(&p.y)
        };
        if r.map_file_name != scope.map_file_name
            || r.map_file_name != "0"
            || r.crosses_map_exit
            || r.kind == ZoneActionKind::MapTransfer
            || !inside(r.origin)
            || !r.path.iter().all(inside)
        {
            return false;
        }
        if scope.status == LifeScopeStatus::Active && r.now_ms < scope.expires_at_ms {
            return true;
        }
        if r.kind != ZoneActionKind::Move || r.path.is_empty() {
            return false;
        }
        self.homeward.allows_live(scope, r.collision, &r.occupancy, r.origin, r.path)
    }
}

impl SharedInProcessZoneRuntimeFactory {
    pub(super) fn configure_companion_boundary(
        &self,
        policy: &CompanionPolicy,
        projection: Option<LifeProjectionHandle>,
    ) -> Result<(), String> {
        let encoded = serde_json::to_vec(policy)
            .map_err(|_| "Invalid companion boundary policy.".to_string())?;
        let validated = CompanionPolicy::parse(&encoded, &policy.realm_id)?;
        {
            let mut slot = self
                .companion_boundary
                .lock()
                .map_err(|_| "Companion boundary configuration unavailable.".to_string())?;
            if let Some(existing) = slot.as_ref() {
                if serde_json::to_vec(existing.policy.as_ref())
                    .map_err(|_| "Companion policy unavailable.".to_string())?
                    != encoded
                {
                    return Err("Companion boundary policy is immutable.".into());
                }
                if let Some(current) = &existing.projection {
                    if projection
                        .as_ref()
                        .is_some_and(|next| !Arc::ptr_eq(current, next))
                    {
                        return Err("Companion life projection cannot be replaced.".into());
                    }
                    // Repeating the party fence setup must not clear life scope.
                    return Ok(());
                }
            }
            *slot = Some(Arc::new(CompanionLifeBoundary {
                policy: Arc::new(validated),
                projection,
                homeward: homeward::HomewardCache::default(),
            }));
        }
        let resources = self
            .zones
            .lock()
            .map_err(|_| "Companion Zones unavailable.".to_string())?;
        for resource in resources.values() {
            let mut state = resource
                .zone_state
                .lock()
                .map_err(|_| "Companion Zone unavailable.".to_string())?;
            self.bind_companion_boundary(&mut state)?;
        }
        Ok(())
    }
    pub(super) fn bind_companion_boundary(
        &self,
        state: &mut SharedInProcessZoneState,
    ) -> Result<(), String> {
        let boundary = self
            .companion_boundary
            .lock()
            .map_err(|_| "Companion boundary configuration unavailable.".to_string())?
            .clone();
        if let Some(boundary) = &boundary {
            state.zone_manager.set_action_boundary(boundary.clone());
        }
        state.companion_boundary = boundary;
        Ok(())
    }
}

/// All enrolled actors can receive a gift but cannot stage outgoing escrow.
pub(crate) fn mandatory_receive_only(packet: &ClientPacket) -> bool {
    matches!(packet, ClientPacket::TradeGold { amount } if *amount > 0)
        || matches!(packet, ClientPacket::DepositTradeItem { .. })
}
/// The first life scope remains restrictive after expiry/revocation until an
/// explicit future host operation releases it. No implicit end-of-task reset.
pub(crate) fn forbidden_during_life(packet: &ClientPacket) -> bool {
    matches!(
        packet,
        ClientPacket::DropItem { .. }
            | ClientPacket::DropGold { .. }
            | ClientPacket::SellItem { .. }
            | ClientPacket::RangeAttack { .. }
            | ClientPacket::Magic { .. }
            | ClientPacket::RepairItem { .. }
            | ClientPacket::SRepairItem { .. }
            | ClientPacket::CraftItem { .. }
            | ClientPacket::BuyItemBack { .. }
            | ClientPacket::ConsignItem { .. }
            | ClientPacket::MarketBuy { .. }
            | ClientPacket::MarketSellNow { .. }
            | ClientPacket::GuildStorageGoldChange { .. }
            | ClientPacket::GuildStorageItemChange { .. }
            | ClientPacket::GameShopBuy { .. }
            | ClientPacket::CallNpc { .. }
            | ClientPacket::NpcConfirmInput { .. }
            | ClientPacket::SendMail { .. }
            | ClientPacket::DepositRefineItem { .. }
            | ClientPacket::RefineItem { .. }
            | ClientPacket::CombineItem { .. }
            | ClientPacket::Awakening { .. }
            | ClientPacket::DisassembleItem { .. }
            | ClientPacket::DowngradeAwakening { .. }
            | ClientPacket::ResetAddedItem { .. }
            | ClientPacket::UpdateIntelligentCreature { .. }
            | ClientPacket::IntelligentCreaturePickup { .. }
            | ClientPacket::NewHero { .. }
            | ClientPacket::SetHeroBehaviour { .. }
            | ClientPacket::ChangeHero { .. }
            | ClientPacket::AcceptQuest { .. }
            | ClientPacket::FinishQuest { .. }
    ) || matches!(packet, ClientPacket::Attack { spell, .. } if *spell != Spell::None)
}

#[cfg(test)]
#[path = "companion_life_boundary_tests.rs"]
mod tests;
