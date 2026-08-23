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
    public void confirmScreenUpSkipsHoverLogicAndUpdatePurgeResumesOnSelection() {
        assertTrue(ChoiceScreenUtils.shouldBlockAfterShopPurgeGridConfirm(
                AbstractDungeon.CurrentScreen.SHOP, true));
        assertFalse(ChoiceScreenUtils.shouldBlockAfterShopPurgeGridConfirm(
                AbstractDungeon.CurrentScreen.SHOP, false));
        assertFalse(ChoiceScreenUtils.shouldBlockAfterShopPurgeGridConfirm(
                AbstractDungeon.CurrentScreen.NONE, true));
        assertFalse(ChoiceScreenUtils.shouldBlockAfterShopPurgeGridConfirm(
                AbstractDungeon.CurrentScreen.MAP, true));

        // desktop-1.0.jar GridCardSelectScreen.update offsets 348–355:
        // confirmScreenUp != 0 jumps over updateCardPositionsAndHoverLogic.
        assertTrue(GridCardSelectScreenPatch.hoverLogicRunsOnThisUpdate(false));
        assertFalse(GridCardSelectScreenPatch.hoverLogicRunsOnThisUpdate(true));
        assertTrue(GridCardSelectScreenPatch.hoverLogicPostfixConsumesChoiceArm(false));
        assertFalse(GridCardSelectScreenPatch.hoverLogicPostfixConsumesChoiceArm(true));

        // CHOOSE frame: hover logic can consume replaceHoverCard.
        GridCardSelectScreenPatch.replaceHoverCard = true;
        if (GridCardSelectScreenPatch.hoverLogicPostfixConsumesChoiceArm(false)) {
            GridCardSelectScreenPatch.replaceHoverCard = false;
        }
        assertFalse(GridCardSelectScreenPatch.replaceHoverCard);

        // CONFIRM frame: hover-logic postfix does not run, so an armed flag
        // would stay set. Do not arm replaceHoverCard on CONFIRM.
        GridCardSelectScreenPatch.replaceHoverCard = true;
        if (GridCardSelectScreenPatch.hoverLogicPostfixConsumesChoiceArm(true)) {
            GridCardSelectScreenPatch.replaceHoverCard = false;
        }
        assertTrue(GridCardSelectScreenPatch.replaceHoverCard);
        GridCardSelectScreenPatch.clearStoredHover();
        assertFalse(GridCardSelectScreenPatch.replaceHoverCard);

        // Vanilla confirm uses the hoveredCard retained after CHOOSE. Restore
        // only if that field is missing, and only on GridCardSelectScreen.update.
        assertFalse(GridCardSelectScreenPatch.shouldRestoreMissingHoveredCardOnConfirmUpdate(
                true, false, true));
        assertTrue(GridCardSelectScreenPatch.shouldRestoreMissingHoveredCardOnConfirmUpdate(
                true, true, true));
        assertFalse(GridCardSelectScreenPatch.shouldRestoreMissingHoveredCardOnConfirmUpdate(
                true, true, false));
        assertFalse(GridCardSelectScreenPatch.shouldRestoreMissingHoveredCardOnConfirmUpdate(
                false, true, true));

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
