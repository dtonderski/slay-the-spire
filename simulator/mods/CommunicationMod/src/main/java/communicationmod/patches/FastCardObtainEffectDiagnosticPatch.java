package communicationmod.patches;

import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePostfixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePrefixPatch;
import com.megacrit.cardcrawl.vfx.FastCardObtainEffect;
import communicationmod.CampfireDiagnostics;

/** Observation-only seam for the delayed card-reward acquire effect. */
@SpirePatch(clz = FastCardObtainEffect.class, method = "update")
public class FastCardObtainEffectDiagnosticPatch {
    @SpirePrefixPatch
    public static void Prefix(FastCardObtainEffect instance) {
        CampfireDiagnostics.effectUpdateEntered("FastCardObtainEffect", instance);
    }

    @SpirePostfixPatch
    public static void Postfix(FastCardObtainEffect instance) {
        CampfireDiagnostics.effectUpdateExited("FastCardObtainEffect", instance);
    }
}
