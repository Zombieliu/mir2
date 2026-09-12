package com.mir2.web3;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class GatewayHostPolicyTest {
    @Test public void startGameRequiresTheFirstAuthoritativeWorldSnapshot() {
        GatewayHostPolicy policy = new GatewayHostPolicy();
        assertEquals(GatewayHostPolicy.Action.NONE,
                policy.observe(GatewaySession.Phase.STARTING, false));
        assertEquals(GatewayHostPolicy.Action.NONE,
                policy.observe(GatewaySession.Phase.IN_GAME, false));
        assertFalse(policy.active());
        assertEquals(GatewayHostPolicy.Action.START,
                policy.observe(GatewaySession.Phase.IN_GAME, true));
        assertTrue(policy.active());
        assertEquals(GatewayHostPolicy.Action.NONE,
                policy.observe(GatewaySession.Phase.IN_GAME, false));
    }

    @Test public void mapTransitionInvalidatesAndRequiresADestinationSnapshot() {
        GatewayHostPolicy policy = new GatewayHostPolicy();
        assertEquals(GatewayHostPolicy.Action.START,
                policy.observe(GatewaySession.Phase.IN_GAME, true));
        assertEquals(GatewayHostPolicy.Action.INVALIDATE,
                policy.observe(GatewaySession.Phase.STARTING, false));
        assertFalse(policy.active());
        assertEquals(GatewayHostPolicy.Action.NONE,
                policy.observe(GatewaySession.Phase.IN_GAME, false));
        assertEquals(GatewayHostPolicy.Action.START,
                policy.observe(GatewaySession.Phase.IN_GAME, true));
        assertTrue(policy.active());
    }

    @Test public void disconnectInvalidatesOnlyOneLiveGeneration() {
        GatewayHostPolicy policy = new GatewayHostPolicy();
        policy.observe(GatewaySession.Phase.IN_GAME, true);
        assertEquals(GatewayHostPolicy.Action.INVALIDATE,
                policy.observe(GatewaySession.Phase.DISCONNECTED, false));
        assertEquals(GatewayHostPolicy.Action.NONE,
                policy.observe(GatewaySession.Phase.DISCONNECTED, false));
        policy.reset();
        assertFalse(policy.active());
    }
}
