package communicationmod.patches;

import basemod.ReflectionHacks;
import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePostfixPatch;
import com.megacrit.cardcrawl.cards.AbstractCard;
import com.megacrit.cardcrawl.screens.select.GridCardSelectScreen;

/**
 * CHOOSE injects {@code hoveredCard} plus a click after
 * {@code updateCardPositionsAndHoverLogic}. Vanilla {@code update()} skips that
 * method when {@code confirmScreenUp} is true (desktop-1.0.jar offset 348–355),
 * then {@code selectedCards.add(hoveredCard)} on confirm-button click
 * (offsets 1420–1485). Later CONFIRM frames therefore never run this postfix.
 */
@SpirePatch(
        clz = GridCardSelectScreen.class,
        method = "updateCardPositionsAndHoverLogic"
)
public class GridCardSelectScreenPatch {

    public static AbstractCard hoverCard;
    public static boolean replaceHoverCard = false;

    public static void armChoice(AbstractCard card) {
        hoverCard = card;
        replaceHoverCard = true;
    }

    /**
     * If {@code hoveredCard} was lost after the preview came up, restore it on
     * {@code GridCardSelectScreen.update} itself — not on hover logic, which
     * does not run on CONFIRM.
     */
    public static boolean shouldRestoreMissingHoveredCardOnConfirmUpdate(
            boolean confirmScreenUp,
            boolean hoveredCardMissing,
            boolean hasStoredHoverCard
    ) {
        return confirmScreenUp && hoveredCardMissing && hasStoredHoverCard;
    }

    public static void clearStoredHover() {
        hoverCard = null;
        replaceHoverCard = false;
    }

    /**
     * Vanilla cancelUpgrade() clears its hoveredCard while returning from the
     * preview to the grid. Discard the matching CommunicationMod selection so
     * a later confirmation cannot restore a card from the cancelled choice.
     */
    @SpirePatch(
            clz = GridCardSelectScreen.class,
            method = "cancelUpgrade"
    )
    public static class CancelUpgradePatch {
        @SpirePostfixPatch
        public static void Postfix(GridCardSelectScreen _instance) {
            clearStoredHover();
        }
    }

    /**
     * Every new grid open calls vanilla callOnOpen(); temporary top-panel
     * overlays call hide()/reopen() instead and must retain the stored choice.
     */
    @SpirePatch(
            clz = GridCardSelectScreen.class,
            method = "callOnOpen"
    )
    public static class NewGridPatch {
        @SpirePostfixPatch
        public static void Postfix(GridCardSelectScreen _instance) {
            clearStoredHover();
        }
    }

    public static void Postfix(GridCardSelectScreen _instance) {
        if (!replaceHoverCard || hoverCard == null) {
            return;
        }
        ReflectionHacks.setPrivate(_instance, GridCardSelectScreen.class, "hoveredCard", hoverCard);
        hoverCard.hb.hovered = true;
        hoverCard.hb.clicked = true;
        replaceHoverCard = false;
    }
}
