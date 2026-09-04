package communicationmod.patches;

import com.badlogic.gdx.math.MathUtils;
import com.evacipated.cardcrawl.modthespire.lib.LineFinder;
import com.evacipated.cardcrawl.modthespire.lib.Matcher;
import com.evacipated.cardcrawl.modthespire.lib.SpireInsertLocator;
import com.evacipated.cardcrawl.modthespire.lib.SpireInsertPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.patcher.PatchingException;
import com.megacrit.cardcrawl.cards.AbstractCard;
import com.megacrit.cardcrawl.cards.CardGroup;
import communicationmod.ExternalRngCapture;
import javassist.CannotCompileException;
import javassist.CtBehavior;

import java.util.ArrayList;

public class CardGroupExternalRngPatch {
    @SpirePatch(
            clz = CardGroup.class,
            method = "getRandomCard",
            paramtypez = {AbstractCard.CardType.class, boolean.class}
    )
    public static class GetRandomCardByTypePatch {
        @SpireInsertPatch(locator = Locator.class)
        public static void Insert(
                CardGroup _instance,
                AbstractCard.CardType cardType,
                boolean useRng
        ) {
            if (!useRng) {
                int candidateCount = 0;
                for (AbstractCard card : _instance.group) {
                    if (card.type == cardType) {
                        candidateCount += 1;
                    }
                }
                ExternalRngCapture.recordCardGroupGetRandomCardByType(candidateCount - 1);
            }
        }

        private static class Locator extends SpireInsertLocator {
            public int[] Locate(CtBehavior method)
                    throws CannotCompileException, PatchingException {
                Matcher matcher = new Matcher.MethodCallMatcher(MathUtils.class, "random");
                return LineFinder.findInOrder(method, new ArrayList<Matcher>(), matcher);
            }
        }
    }
}
