package communicationmod.patches;

import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePostfixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePrefixPatch;
import com.megacrit.cardcrawl.vfx.ObtainKeyEffect;
import communicationmod.CampfireDiagnostics;

/** Observation-only seam for the delayed key grant used by Recall. */
@SpirePatch(clz = ObtainKeyEffect.class, method = "update")
public class ObtainKeyEffectDiagnosticPatch {
    @SpirePrefixPatch
    public static void Prefix(ObtainKeyEffect instance) {
        CampfireDiagnostics.effectUpdateEntered("ObtainKeyEffect", instance);
    }

    @SpirePostfixPatch
    public static void Postfix(ObtainKeyEffect instance) {
        CampfireDiagnostics.effectUpdateExited("ObtainKeyEffect", instance);
    }
}
