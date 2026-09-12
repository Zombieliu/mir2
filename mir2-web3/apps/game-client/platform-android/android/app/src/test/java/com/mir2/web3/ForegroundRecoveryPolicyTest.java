package com.mir2.web3;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class ForegroundRecoveryPolicyTest {
    @Test public void initialStartNeverDuplicatesTheOnCreateConnection() {
        assertFalse(new ForegroundRecoveryPolicy().takeReconnectOnStart());
    }

    @Test public void productionStopDisconnectsAndReconnectsExactlyOnce() {
        ForegroundRecoveryPolicy policy = new ForegroundRecoveryPolicy();
        assertTrue(policy.markStoppedAndShouldDisconnect(false));
        assertTrue(policy.takeReconnectOnStart());
        assertFalse(policy.takeReconnectOnStart());
    }

    @Test public void previewStopPreservesTheOfflineUiFixture() {
        ForegroundRecoveryPolicy policy = new ForegroundRecoveryPolicy();
        assertFalse(policy.markStoppedAndShouldDisconnect(true));
        assertFalse(policy.takeReconnectOnStart());
    }

    @Test public void newestStopModeReplacesAnOlderPendingDecision() {
        ForegroundRecoveryPolicy policy = new ForegroundRecoveryPolicy();
        assertTrue(policy.markStoppedAndShouldDisconnect(false));
        assertFalse(policy.markStoppedAndShouldDisconnect(true));
        assertFalse(policy.takeReconnectOnStart());
    }
}
