package communicationmod;

import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import communicationmod.patches.ShopRoomPurgePatch;
import org.junit.Test;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

public class ShopPurgeConfirmAckTest {

    @Test
    public void shopPurgeConfirmBlocksUntilUpdatePurgeProcessesSelection() {
        assertTrue(ChoiceScreenUtils.shouldBlockAfterShopPurgeGridConfirm(
                AbstractDungeon.CurrentScreen.SHOP, true));
        assertFalse(ChoiceScreenUtils.shouldBlockAfterShopPurgeGridConfirm(
                AbstractDungeon.CurrentScreen.SHOP, false));
        assertFalse(ChoiceScreenUtils.shouldBlockAfterShopPurgeGridConfirm(
                AbstractDungeon.CurrentScreen.NONE, true));
        assertFalse(ChoiceScreenUtils.shouldBlockAfterShopPurgeGridConfirm(
                AbstractDungeon.CurrentScreen.REST, true));

        assertFalse(ShopRoomPurgePatch.shouldResumeAfterShopUpdatePurge(false, false));
        assertFalse(ShopRoomPurgePatch.shouldResumeAfterShopUpdatePurge(false, true));
        assertFalse(ShopRoomPurgePatch.shouldResumeAfterShopUpdatePurge(true, false));
        assertTrue(ShopRoomPurgePatch.shouldResumeAfterShopUpdatePurge(true, true));

        GameStateListener.resetStateVariables();
        assertEquals(6, GameStateListener.getBoundarySchema());
        assertFalse(GameStateListener.isStateUpdateBlocked());

        GameStateListener.blockStateUpdate();
        ShopRoomPurgePatch.resumeListenerIfShopPurgeCompleted(false, true);
        assertTrue(GameStateListener.isStateUpdateBlocked());

        ShopRoomPurgePatch.resumeListenerIfShopPurgeCompleted(true, false);
        assertTrue(GameStateListener.isStateUpdateBlocked());

        ShopRoomPurgePatch.resumeListenerIfShopPurgeCompleted(true, true);
        assertFalse(GameStateListener.isStateUpdateBlocked());
        assertFalse(GameStateListener.hasCompletingBoundary());
    }
}
