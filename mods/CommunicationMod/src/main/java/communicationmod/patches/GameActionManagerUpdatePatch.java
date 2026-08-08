package communicationmod.patches;

import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePrefixPatch;
import com.megacrit.cardcrawl.actions.GameActionManager;
import communicationmod.GameStateListener;

/** Records the exact GameActionManager path that invokes currentAction.update(). */
@SpirePatch(
        clz = GameActionManager.class,
        method = "update"
)
public class GameActionManagerUpdatePatch {
    @SpirePrefixPatch
    public static void Prefix(GameActionManager instance) {
        if (instance.phase == GameActionManager.Phase.EXECUTING_ACTIONS
                && instance.currentAction != null
                && !instance.currentAction.isDone) {
            GameStateListener.signalCurrentActionUpdate(instance.currentAction);
        }
    }
}
