package com.mir2.web3;

import org.junit.Test;
import static org.junit.Assert.assertEquals;

public final class EditorBackPolicyTest {
    @Test public void imeDismissalBetweenDownAndUpDoesNotCancelTheSharedDraft() {
        EditorBackPolicy policy = new EditorBackPolicy();
        policy.onKeyDown(true, 0);
        assertEquals(EditorBackPolicy.Action.DISMISS_EDITOR, policy.onKeyUp(true, false, false));
        policy.onKeyDown(false, 0);
        assertEquals(EditorBackPolicy.Action.BACK_TO_SHARED_UI, policy.onKeyUp(true, false, false));
    }

    @Test public void repeatsDoNotOverwriteTheOriginalEditorOwner() {
        EditorBackPolicy policy = new EditorBackPolicy();
        policy.onKeyDown(true, 0);
        policy.onKeyDown(false, 1);
        assertEquals(EditorBackPolicy.Action.DISMISS_EDITOR, policy.onKeyUp(true, false, false));
    }

    @Test public void cancelledAndUntrackedKeysDoNothingAndReleaseOwnership() {
        EditorBackPolicy policy = new EditorBackPolicy();
        policy.onKeyDown(true, 0);
        assertEquals(EditorBackPolicy.Action.NONE, policy.onKeyUp(true, true, false));
        assertEquals(EditorBackPolicy.Action.NONE, policy.onKeyUp(false, false, true));
        assertEquals(EditorBackPolicy.Action.BACK_TO_SHARED_UI, policy.onKeyUp(true, false, false));
    }

    @Test public void newlyActiveEditorStillConsumesTheGesture() {
        EditorBackPolicy policy = new EditorBackPolicy();
        policy.onKeyDown(false, 0);
        assertEquals(EditorBackPolicy.Action.DISMISS_EDITOR, policy.onKeyUp(true, false, true));
    }

    @Test public void backgroundResetCannotRetainAnOldBackOwner() {
        EditorBackPolicy policy = new EditorBackPolicy();
        policy.onKeyDown(true, 0);
        policy.reset();
        assertEquals(EditorBackPolicy.Action.BACK_TO_SHARED_UI, policy.onKeyUp(true, false, false));
    }
}
