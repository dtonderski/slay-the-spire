package communicationmod.patches;

import basemod.ReflectionHacks;
import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.megacrit.cardcrawl.cards.AbstractCard;
import com.megacrit.cardcrawl.screens.select.GridCardSelectScreen;

@SpirePatch(
        clz = GridCardSelectScreen.class,
        method = "updateCardPositionsAndHoverLogic"
)
public class GridCardSelectScreenPatch {

    public static AbstractCard hoverCard;
    public static boolean replaceHoverCard = false;
    public static boolean clickHoveredCard = false;

    public static void armChoice(AbstractCard card) {
        hoverCard = card;
        replaceHoverCard = true;
        clickHoveredCard = true;
    }

    /**
     * Vanilla confirm adds {@code hoveredCard} to {@code selectedCards}. CHOOSE
     * only injects that hover for one {@code updateCardPositionsAndHoverLogic}
     * call. Later CONFIRM frames recompute hover from the cursor, which is not
     * over the card, so {@code hoveredCard} is null and the purge never starts.
     * Re-apply the stored card after hover logic and do not click it again.
     */
    public static boolean shouldRetainHoveredCardForConfirm(
            boolean confirmScreenUp,
            boolean hasStoredHoverCard
    ) {
        return confirmScreenUp && hasStoredHoverCard;
    }

    public static void retainHoveredCardForConfirm(boolean confirmScreenUp) {
        if (shouldRetainHoveredCardForConfirm(confirmScreenUp, hoverCard != null)) {
            replaceHoverCard = true;
            clickHoveredCard = false;
        }
    }

    public static void Postfix(GridCardSelectScreen _instance) {
        if (replaceHoverCard && hoverCard != null) {
            ReflectionHacks.setPrivate(_instance, GridCardSelectScreen.class, "hoveredCard", hoverCard);
            hoverCard.hb.hovered = true;
            if (clickHoveredCard) {
                hoverCard.hb.clicked = true;
            }
            replaceHoverCard = false;
            clickHoveredCard = false;
        }
    }
}
