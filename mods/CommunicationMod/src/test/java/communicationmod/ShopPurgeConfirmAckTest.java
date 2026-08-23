package communicationmod;

import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import communicationmod.patches.GridCardSelectScreenPatch;
import communicationmod.patches.ShopScreenPatch;
import org.junit.Test;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

public class ShopPurgeConfirmAckTest {

    @Test
    public void shopPurgeConfirmRetainsHoverAndResumesAfterShopScreenUpdatePurge() {
        assertTrue(ChoiceScreenUtils.shouldBlockAfterShopPurgeGridConfirm(
                AbstractDungeon.CurrentScreen.SHOP, true));
        assertFalse(ChoiceScreenUtils.shouldBlockAfterShopPurgeGridConfirm(
                AbstractDungeon.CurrentScreen.SHOP, false));
        assertFalse(ChoiceScreenUtils.shouldBlockAfterShopPurgeGridConfirm(
                AbstractDungeon.CurrentScreen.NONE, true));
        assertFalse(ChoiceScreenUtils.shouldBlockAfterShopPurgeGridConfirm(
                AbstractDungeon.CurrentScreen.MAP, true));

        assertTrue(GridCardSelectScreenPatch.shouldRetainHoveredCardForConfirm(true, true));
        assertFalse(GridCardSelectScreenPatch.shouldRetainHoveredCardForConfirm(true, false));
        assertFalse(GridCardSelectScreenPatch.shouldRetainHoveredCardForConfirm(false, true));

        assertFalse(ShopScreenPatch.UpdatePurgePatch.shouldResumeAfterShopScreenUpdatePurge(false, false));
        assertFalse(ShopScreenPatch.UpdatePurgePatch.shouldResumeAfterShopScreenUpdatePurge(false, true));
        assertFalse(ShopScreenPatch.UpdatePurgePatch.shouldResumeAfterShopScreenUpdatePurge(true, false));
        assertTrue(ShopScreenPatch.UpdatePurgePatch.shouldResumeAfterShopScreenUpdatePurge(true, true));

        GameStateListener.resetStateVariables();
        assertEquals(6, GameStateListener.getBoundarySchema());
        assertFalse(GameStateListener.isStateUpdateBlocked());

        GameStateListener.blockStateUpdate();
        if (ShopScreenPatch.UpdatePurgePatch.shouldResumeAfterShopScreenUpdatePurge(false, true)) {
            GameStateListener.resumeStateUpdate();
        }
        assertTrue(GameStateListener.isStateUpdateBlocked());

        if (ShopScreenPatch.UpdatePurgePatch.shouldResumeAfterShopScreenUpdatePurge(true, false)) {
            GameStateListener.resumeStateUpdate();
        }
        assertTrue(GameStateListener.isStateUpdateBlocked());

        if (ShopScreenPatch.UpdatePurgePatch.shouldResumeAfterShopScreenUpdatePurge(true, true)) {
            GameStateListener.resumeStateUpdate();
        }
        assertFalse(GameStateListener.isStateUpdateBlocked());
        assertFalse(GameStateListener.hasCompletingBoundary());
    }
}
