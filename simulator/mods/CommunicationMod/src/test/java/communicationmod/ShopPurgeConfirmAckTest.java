package communicationmod;

import basemod.ReflectionHacks;
import com.megacrit.cardcrawl.cards.AbstractCard;
import com.megacrit.cardcrawl.characters.AbstractPlayer;
import com.megacrit.cardcrawl.core.CardCrawlGame;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.helpers.Hitbox;
import com.megacrit.cardcrawl.localization.LocalizedStrings;
import com.megacrit.cardcrawl.localization.UIStrings;
import com.megacrit.cardcrawl.monsters.AbstractMonster;
import com.megacrit.cardcrawl.screens.select.GridCardSelectScreen;
import communicationmod.patches.GridCardSelectScreenPatch;
import communicationmod.patches.GridCardSelectScreenUpdatePatch;
import communicationmod.patches.ShopScreenPatch;
import org.junit.Test;
import sun.misc.Unsafe;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Arrays;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertSame;
import static org.junit.Assert.assertTrue;

public class ShopPurgeConfirmAckTest {

    private static Unsafe getUnsafe() throws ReflectiveOperationException {
        Field field = Unsafe.class.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        return (Unsafe) field.get(null);
    }

    private static void installTestLocalization() throws ReflectiveOperationException {
        TestLocalizedStrings languagePack = (TestLocalizedStrings) getUnsafe().allocateInstance(
                TestLocalizedStrings.class);
        UIStrings uiStrings = new UIStrings();
        uiStrings.TEXT = new String[100];
        Arrays.fill(uiStrings.TEXT, "test");
        languagePack.uiStrings = uiStrings;
        CardCrawlGame.languagePack = languagePack;
    }

    private static GridCardSelectScreen emptyGridScreen() throws ReflectiveOperationException {
        GridCardSelectScreen screen = (GridCardSelectScreen) getUnsafe().allocateInstance(
                GridCardSelectScreen.class);
        screen.selectedCards = new ArrayList<>();
        return screen;
    }

    private static TestCard emptyCard() throws ReflectiveOperationException {
        TestCard card = (TestCard) getUnsafe().allocateInstance(TestCard.class);
        card.hb = new Hitbox(1.0f, 1.0f);
        return card;
    }

    private static class TestLocalizedStrings extends LocalizedStrings {
        private UIStrings uiStrings;

        @Override
        public UIStrings getUIString(String key) {
            return uiStrings;
        }
    }

    private static class TestCard extends AbstractCard {
        private TestCard() {
            super("test", "test", null, 0, "", CardType.SKILL, CardColor.COLORLESS,
                    CardRarity.SPECIAL, CardTarget.NONE);
        }

        @Override
        public void upgrade() {
        }

        @Override
        public void use(AbstractPlayer player, AbstractMonster monster) {
        }

        @Override
        public AbstractCard makeCopy() {
            return this;
        }
    }

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
        assertEquals(7, GameStateListener.getBoundarySchema());
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

    @Test
    public void productionPatchesCarrySelectionThroughConfirmAndPurge() throws Exception {
        LocalizedStrings oldLanguagePack = CardCrawlGame.languagePack;
        installTestLocalization();
        GridCardSelectScreen screen = emptyGridScreen();
        TestCard card = emptyCard();
        try {
            screen.confirmScreenUp = true;
            screen.forPurge = true;
            ReflectionHacks.setPrivate(screen, GridCardSelectScreen.class, "hoveredCard", null);

            // State left by the CHOOSE frame after the hover-logic postfix has
            // clicked this card and vanilla has opened its purge preview.
            GridCardSelectScreenPatch.armChoice(card);
            GridCardSelectScreenPatch.replaceHoverCard = false;

            GridCardSelectScreenUpdatePatch.Prefix(screen);
            AbstractCard restored = (AbstractCard) ReflectionHacks.getPrivate(
                    screen, GridCardSelectScreen.class, "hoveredCard");
            assertSame(card, restored);
            assertTrue(card.hb.hovered);

            // Model vanilla's CONFIRM branch, which adds hoveredCard before it
            // closes the grid. The production postfix must retire stored state.
            screen.confirmScreenUp = false;
            screen.selectedCards.add(restored);
            GridCardSelectScreenUpdatePatch.Postfix(screen);
            assertNull(GridCardSelectScreenPatch.hoverCard);
            assertFalse(GridCardSelectScreenPatch.replaceHoverCard);

            GameStateListener.resetStateVariables();
            GameStateListener.blockStateUpdate();
            ShopScreenPatch.UpdatePurgePatch.recordSelectionBeforeUpdate(screen);
            screen.selectedCards.clear(); // Vanilla updatePurge consumes it.
            ShopScreenPatch.UpdatePurgePatch.Postfix(null);
            assertFalse(GameStateListener.isStateUpdateBlocked());
        } finally {
            GridCardSelectScreenPatch.clearStoredHover();
            GameStateListener.resetStateVariables();
            CardCrawlGame.languagePack = oldLanguagePack;
        }
    }

    @Test
    public void gridCancelAndNewGridOpenClearStoredSelection() throws Exception {
        LocalizedStrings oldLanguagePack = CardCrawlGame.languagePack;
        installTestLocalization();
        GridCardSelectScreen screen = emptyGridScreen();
        TestCard card = emptyCard();
        try {
            GridCardSelectScreenPatch.armChoice(card);
            GridCardSelectScreenPatch.CancelUpgradePatch.Postfix(screen);
            assertNull(GridCardSelectScreenPatch.hoverCard);
            assertFalse(GridCardSelectScreenPatch.replaceHoverCard);

            GridCardSelectScreenPatch.armChoice(card);
            GridCardSelectScreenPatch.NewGridPatch.Postfix(screen);
            assertNull(GridCardSelectScreenPatch.hoverCard);
            assertFalse(GridCardSelectScreenPatch.replaceHoverCard);
        } finally {
            GridCardSelectScreenPatch.clearStoredHover();
            CardCrawlGame.languagePack = oldLanguagePack;
        }
    }
}
