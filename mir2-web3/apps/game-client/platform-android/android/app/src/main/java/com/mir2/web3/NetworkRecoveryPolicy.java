package com.mir2.web3;

/** One-shot decisions for real Android default-network changes. */
final class NetworkRecoveryPolicy {
    enum Action { NONE, CONNECT, DISCONNECT }

    private Boolean available;
    private boolean reconnectWhenOnline;

    Action onNetworkChanged(boolean nextAvailable, boolean foreground, boolean uiPreview) {
        if (available != null && available == nextAvailable) return Action.NONE;
        available = nextAvailable;
        if (uiPreview) return Action.NONE;
        if (!nextAvailable) {
            reconnectWhenOnline = true;
            return Action.DISCONNECT;
        }
        if (foreground && reconnectWhenOnline) {
            reconnectWhenOnline = false;
            return Action.CONNECT;
        }
        return Action.NONE;
    }

    Action onForeground(boolean reconnectRequested, boolean uiPreview) {
        if (uiPreview) return Action.NONE;
        reconnectWhenOnline |= reconnectRequested;
        if (Boolean.FALSE.equals(available)) return Action.NONE;
        if (reconnectWhenOnline) {
            reconnectWhenOnline = false;
            return Action.CONNECT;
        }
        return Action.NONE;
    }
}
