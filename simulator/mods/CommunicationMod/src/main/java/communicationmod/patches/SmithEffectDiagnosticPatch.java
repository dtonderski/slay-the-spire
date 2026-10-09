package communicationmod.patches;

import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePrefixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePostfixPatch;
import com.megacrit.cardcrawl.vfx.campfire.CampfireSmithEffect;
import communicationmod.CampfireDiagnostics;

/** Opt-in observations only. Prefix ordering relative to other patches is not assumed. */
@SpirePatch(clz = CampfireSmithEffect.class, method = "update")
public class SmithEffectDiagnosticPatch {
    @SpirePrefixPatch
    public static void before(CampfireSmithEffect effect) {
        CampfireDiagnostics.effectUpdateEntered("CampfireSmithEffect", effect);
    }

    @SpirePostfixPatch
    public static void after(CampfireSmithEffect effect) {
        CampfireDiagnostics.effectUpdateExited("CampfireSmithEffect", effect);
    }
}
