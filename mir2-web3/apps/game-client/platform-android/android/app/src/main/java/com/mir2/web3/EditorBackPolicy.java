package com.mir2.web3;

/** One Back gesture cannot both dismiss the IME and cancel a shared draft. */
final class EditorBackPolicy {
    enum Action { NONE, DISMISS_EDITOR, BACK_TO_SHARED_UI }
    private boolean startedInEditor;

    void onKeyDown(boolean editorActive, int repeatCount) {
        if (repeatCount == 0) startedInEditor = editorActive;
    }

    Action onKeyUp(boolean tracked, boolean cancelled, boolean editorActive) {
        boolean dismissEditor = startedInEditor || editorActive;
        startedInEditor = false;
        if (!tracked || cancelled) return Action.NONE;
        return dismissEditor ? Action.DISMISS_EDITOR : Action.BACK_TO_SHARED_UI;
    }

    void reset() { startedInEditor = false; }
}
