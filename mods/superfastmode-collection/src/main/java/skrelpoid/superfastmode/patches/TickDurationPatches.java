package skrelpoid.superfastmode.patches;

import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePrefixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpireReturn;
import com.megacrit.cardcrawl.actions.AbstractGameAction;

/**
 * Keeps gameplay action state machines on deterministic canonical 60 Hz ticks.
 *
 * The collection fork globally multiplies libGDX delta so visual transitions
 * can run quickly. AbstractGameAction.tickDuration is also fed by that global
 * delta in the target game. At 100x an action can therefore expire in the same
 * update that opens an input screen, before the later update that retrieves the
 * selected cards. A fixed 60 Hz gameplay delta preserves multi-update ordering
 * while uncapped rendering still executes those updates quickly.
 */
public class TickDurationPatches {

    @SpirePatch(clz = AbstractGameAction.class, method = "tickDuration")
    public static class DeterministicGameplayTick {
        @SpirePrefixPatch
        public static SpireReturn<Void> Prefix(AbstractGameAction __instance) {
            skrelpoid.superfastmode.SuperFastMode.tickGameplayDuration(__instance);
            return SpireReturn.Return(null);
        }
    }
}
