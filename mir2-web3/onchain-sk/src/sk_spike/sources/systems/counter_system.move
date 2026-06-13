// SK0 spike — a hand-written system that exercises a Dubhe 1.2.x session key.
//
// `bump` writes the PER-USER `counter` resource, which lives in the caller's
// `UserStorage`. The framework's `dapp_system::set_record` (called inside
// `counter::set`) enforces `dapp_service::is_write_authorized(us, ctx.sender(), now)`:
//   - ctx.sender() == us.canonical_owner            → allowed (the owner), OR
//   - ctx.sender() == us.session_key && not expired → allowed (an active session key).
// Either way the data updated is the OWNER's, because it is the owner's UserStorage.
// So an ephemeral session wallet can `bump` on the owner's behalf with NO wallet popup,
// and `dapp_service::canonical_owner(us)` is the address we'd stamp on an event.
module sk_spike::counter_system {
    use dubhe::dapp_service::UserStorage;
    use sk_spike::counter;

    /// Increment the counter belonging to `user_storage`'s canonical owner by one.
    /// `public fun` (not `entry`) so a PTB — signed by the session wallet — can call it.
    public fun bump(user_storage: &mut UserStorage, ctx: &mut TxContext) {
        let current = if (counter::has(user_storage)) { counter::get(user_storage) } else { 0 };
        counter::set(user_storage, current + 1, ctx);
    }
}
