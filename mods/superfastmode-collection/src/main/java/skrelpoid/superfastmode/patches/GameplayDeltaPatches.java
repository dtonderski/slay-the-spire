package skrelpoid.superfastmode.patches;

import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import javassist.CannotCompileException;
import javassist.expr.ExprEditor;
import javassist.expr.MethodCall;

/**
 * Covers target action implementations that subtract getDeltaTime directly
 * instead of delegating to AbstractGameAction.tickDuration.
 *
 * This list comes from a target-jar constant-pool audit of every class below
 * com.megacrit.cardcrawl.actions. Keeping it explicit makes additions in a new
 * game build auditable instead of changing visual delta globally.
 */
public class GameplayDeltaPatches {

    @SpirePatch(clz = com.megacrit.cardcrawl.actions.common.ApplyPoisonOnRandomMonsterAction.class, method = "update")
    @SpirePatch(clz = com.megacrit.cardcrawl.actions.common.ApplyPowerAction.class, method = "update")
    @SpirePatch(clz = com.megacrit.cardcrawl.actions.common.DrawCardAction.class, method = "update")
    @SpirePatch(clz = com.megacrit.cardcrawl.actions.common.FastDrawCardAction.class, method = "update")
    @SpirePatch(clz = com.megacrit.cardcrawl.actions.common.MakeTempCardAtBottomOfDeckAction.class, method = "update")
    @SpirePatch(clz = com.megacrit.cardcrawl.actions.common.MakeTempCardInDiscardAction.class, method = "update")
    @SpirePatch(clz = com.megacrit.cardcrawl.actions.common.MakeTempCardInDrawPileAction.class, method = "update")
    @SpirePatch(clz = com.megacrit.cardcrawl.actions.defect.ScrapeAction.class, method = "update")
    @SpirePatch(clz = com.megacrit.cardcrawl.actions.unique.RipAndTearAction.class, method = "update")
    public static class DeterministicDirectGameplayDelta {
        public static ExprEditor Instrument() {
            return new ExprEditor() {
                @Override
                public void edit(MethodCall method) throws CannotCompileException {
                    if (method.getMethodName().equals("getDeltaTime")) {
                        method.replace("{ $_ = skrelpoid.superfastmode.SuperFastMode.getGameplayDelta(); }");
                    }
                }
            };
        }
    }
}
