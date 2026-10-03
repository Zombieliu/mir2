package com.mir2.web3;

/**
 * One-shot Activity foreground recovery decision, kept independent of Android
 * framework classes so the security/lifecycle contract has local JVM tests.
 */
final class ForegroundRecoveryPolicy {
    private boolean reconnectOnStart;

    /** Returns whether the authenticated transport must be discarded now. */
    boolean markStoppedAndShouldDisconnect(boolean uiPreview) {
        reconnectOnStart = !uiPreview;
        return !uiPreview;
    }

    /** Returns true exactly once after a production stop. */
    boolean takeReconnectOnStart() {
        boolean reconnect = reconnectOnStart;
        reconnectOnStart = false;
        return reconnect;
    }
}
