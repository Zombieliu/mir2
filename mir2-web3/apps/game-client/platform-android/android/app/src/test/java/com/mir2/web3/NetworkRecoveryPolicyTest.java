package com.mir2.web3;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public class NetworkRecoveryPolicyTest {
    @Test public void foregroundReconnectWaitsForTheRealNetwork() {
        NetworkRecoveryPolicy policy = new NetworkRecoveryPolicy();
        assertEquals(NetworkRecoveryPolicy.Action.DISCONNECT,
                policy.onNetworkChanged(false, true, false));
        assertEquals(NetworkRecoveryPolicy.Action.NONE,
                policy.onForeground(true, false));
        assertEquals(NetworkRecoveryPolicy.Action.CONNECT,
                policy.onNetworkChanged(true, true, false));
        assertEquals(NetworkRecoveryPolicy.Action.NONE,
                policy.onNetworkChanged(true, true, false));
    }

    @Test public void recoveryWaitsWhileTheActivityIsBackgrounded() {
        NetworkRecoveryPolicy policy = new NetworkRecoveryPolicy();
        policy.onNetworkChanged(false, false, false);
        assertEquals(NetworkRecoveryPolicy.Action.NONE,
                policy.onNetworkChanged(true, false, false));
        assertEquals(NetworkRecoveryPolicy.Action.CONNECT,
                policy.onForeground(false, false));
    }

    @Test public void previewNeverStartsOrStopsATransport() {
        NetworkRecoveryPolicy policy = new NetworkRecoveryPolicy();
        assertEquals(NetworkRecoveryPolicy.Action.NONE,
                policy.onNetworkChanged(false, true, true));
        assertEquals(NetworkRecoveryPolicy.Action.NONE,
                policy.onNetworkChanged(true, true, true));
        assertEquals(NetworkRecoveryPolicy.Action.NONE,
                policy.onForeground(true, true));
    }
}
