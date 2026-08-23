package communicationmod.patches;

import basemod.ReflectionHacks;
import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
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
     * Bytecode: {@code confirmScreenUp != 0} jumps over
     * {@code updateCardPositionsAndHoverLogic}. CHOOSE is the only frame that
     * can consume {@code replaceHoverCard} here.
     */
    public static boolean hoverLogicRunsOnThisUpdate(boolean confirmScreenUp) {
        return !confirmScreenUp;
    }

    public static boolean hoverLogicPostfixConsumesChoiceArm(boolean confirmScreenUp) {
        return hoverLogicRunsOnThisUpdate(confirmScreenUp);
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
