package com.mir2.web3;

/**
 * Gates Rust-produced gameplay writes on a renderable authoritative scene.
 * Transport connectivity alone is insufficient: StartGame and every map
 * transition must deliver a new world snapshot before a fresh host generation
 * may send gameplay commands.
 */
final class GatewayHostPolicy {
    enum Action { NONE, START, INVALIDATE }

    private volatile boolean active;

    Action observe(GatewaySession.Phase phase, boolean hasWorldSnapshot) {
        if (phase == GatewaySession.Phase.IN_GAME) {
            if (!active && hasWorldSnapshot) {
                active = true;
                return Action.START;
            }
            return Action.NONE;
        }
        if (active) {
            active = false;
            return Action.INVALIDATE;
        }
        return Action.NONE;
    }

    boolean active() { return active; }

    void reset() { active = false; }
}
