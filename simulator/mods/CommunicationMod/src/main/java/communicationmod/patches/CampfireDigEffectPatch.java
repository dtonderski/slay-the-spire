package communicationmod.patches;

import com.evacipated.cardcrawl.modthespire.lib.*;
import com.evacipated.cardcrawl.modthespire.patcher.PatchingException;
import com.megacrit.cardcrawl.metrics.MetricData;
import com.megacrit.cardcrawl.vfx.campfire.CampfireDigEffect;
import communicationmod.GameStateListener;
import javassist.CannotCompileException;
import javassist.CtBehavior;

import java.util.ArrayList;

@SpirePatch(
        clz= CampfireDigEffect.class,
        method="update"
)
public class CampfireDigEffectPatch {

    @SpirePrefixPatch
    public static void Prefix(CampfireDigEffect _instance) {
        communicationmod.CampfireDiagnostics.effectUpdateEntered("CampfireDigEffect", _instance);
    }

    @SpirePostfixPatch
    public static void Postfix(CampfireDigEffect _instance) {
        communicationmod.CampfireDiagnostics.effectUpdateExited("CampfireDigEffect", _instance);
    }

    @SpireInsertPatch(
            locator=LocatorAfter.class
    )
    public static void After(CampfireDigEffect _instance) {
        GameStateListener.resumeStateUpdate();
    }

    private static class LocatorAfter extends SpireInsertLocator {
        public int[] Locate(CtBehavior ctMethodToPatch) throws CannotCompileException, PatchingException {
            Matcher matcher = new Matcher.FieldAccessMatcher(CampfireDigEffect.class, "isDone");
            return LineFinder.findInOrder(ctMethodToPatch, new ArrayList<Matcher>(), matcher);
        }
    }

    @SpireInsertPatch(
            locator=LocatorBefore.class
    )
    public static void Before(CampfireDigEffect _instance) {
        GameStateListener.blockStateUpdate();
    }

    private static class LocatorBefore extends SpireInsertLocator {
        public int[] Locate(CtBehavior ctMethodToPatch) throws CannotCompileException, PatchingException {
            Matcher matcher = new Matcher.MethodCallMatcher(MetricData.class, "addCampfireChoiceData");
            return LineFinder.findInOrder(ctMethodToPatch, new ArrayList<Matcher>(), matcher);
        }
    }
}
