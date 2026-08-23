package communicationmod.patches;

import basemod.ReflectionHacks;
import com.evacipated.cardcrawl.modthespire.lib.*;
import com.evacipated.cardcrawl.modthespire.patcher.PatchingException;
import com.megacrit.cardcrawl.cards.AbstractCard;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.shop.ShopScreen;
import communicationmod.GameStateListener;
import javassist.CannotCompileException;
import javassist.CtBehavior;

import java.util.ArrayList;

public class ShopScreenPatch {

    public static boolean doHover = false;
    public static AbstractCard hoverCard;

    /**
     * Vanilla shop-purge completion: Grid confirm returns to SHOP, then
     * {@code ShopScreen.update()} calls {@code updatePurge()}, which spends gold
     * via {@code purgeCard()} and removes {@code selectedCards}.
     * {@code ShopScreen.purchasePurge()} is what opens the grid.
     *
     * Resume only when this call started with a real purge selection so CHOOSE
     * purge (empty {@code selectedCards}) does not ack before CONFIRM.
     */
    @SpirePatch(
            clz = ShopScreen.class,
            method = "updatePurge"
    )
    public static class UpdatePurgePatch {

        private static boolean resumeAfterThisUpdate = false;

        public static boolean shouldResumeAfterShopScreenUpdatePurge(
                boolean hadSelectedCards,
                boolean forPurge
        ) {
            return hadSelectedCards && forPurge;
        }

        public static void Prefix(ShopScreen _instance) {
            boolean hadSelectedCards = !AbstractDungeon.gridSelectScreen.selectedCards.isEmpty();
            boolean forPurge = AbstractDungeon.gridSelectScreen.forPurge;
            resumeAfterThisUpdate = shouldResumeAfterShopScreenUpdatePurge(hadSelectedCards, forPurge);
        }

        public static void Postfix(ShopScreen _instance) {
            if (resumeAfterThisUpdate) {
                resumeAfterThisUpdate = false;
                GameStateListener.resumeStateUpdate();
            }
        }
    }

    @SpirePatch(
            clz=ShopScreen.class,
            method = "update"
    )
    public static class HoverCardPatch {

        @SuppressWarnings("unchecked")
        @SpireInsertPatch(
                locator=Locator.class
        )
        public static void Insert(ShopScreen _instance) {
            if(doHover) {
                ArrayList<AbstractCard> coloredCards = (ArrayList<AbstractCard>) ReflectionHacks.getPrivate(_instance, ShopScreen.class, "coloredCards");
                ArrayList<AbstractCard> colorlessCards = (ArrayList<AbstractCard>) ReflectionHacks.getPrivate(_instance, ShopScreen.class, "colorlessCards");
                for(AbstractCard card : coloredCards) {
                    card.hb.hovered = card == hoverCard;
                }
                for(AbstractCard card : colorlessCards) {
                    card.hb.hovered = card == hoverCard;
                }
                doHover = false;
            }
        }

        private static class Locator extends SpireInsertLocator {
            public int[] Locate(CtBehavior ctMethodToPatch) throws CannotCompileException, PatchingException {
                Matcher matcher = new Matcher.MethodCallMatcher(ShopScreen.class, "updateHand");
                return LineFinder.findInOrder(ctMethodToPatch, new ArrayList<Matcher>(), matcher);
            }
        }

    }

}
