package abandonruncontrol;

import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePostfixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePrefixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpireReturn;
import com.megacrit.cardcrawl.core.CardCrawlGame;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.saveAndContinue.SaveAndContinue;
import communicationmod.CommandExecutor;
import java.util.ArrayList;

public final class AbandonRunControl {
    private static final String ABANDON = "abandon";

    private AbandonRunControl() {
    }

    public static boolean isAbandonCommand(String command) {
        if (command == null) {
            return false;
        }
        String[] parts = command.trim().toLowerCase().split("\\s+");
        return parts.length > 0 && ABANDON.equals(parts[0]);
    }

    public static boolean isInAbandonableRun() {
        return CardCrawlGame.mode == CardCrawlGame.GameMode.GAMEPLAY
            && AbstractDungeon.player != null;
    }

    public static void abandonRun() {
        abandonRunNow();
    }

    private static void abandonRunNow() {
        if (!isInAbandonableRun()) {
            return;
        }

        try {
            if (AbstractDungeon.getCurrRoom() != null) {
                AbstractDungeon.getCurrRoom().clearEvent();
            }
        } catch (RuntimeException ignored) {
        }

        try {
            if (AbstractDungeon.player.stance != null) {
                AbstractDungeon.player.stance.stopIdleSfx();
            }
        } catch (RuntimeException ignored) {
        }

        try {
            SaveAndContinue.deleteSave(AbstractDungeon.player);
        } catch (RuntimeException ignored) {
        }

        try {
            AbstractDungeon.closeCurrentScreen();
        } catch (RuntimeException ignored) {
        }

        // Use the game's own fade/reset lifecycle. It clears all dungeon and
        // seed globals before constructing the main menu, which makes a later
        // CommunicationMod START behave exactly like a normal new run.
        CardCrawlGame.startOver();
    }

    @SpirePatch(clz = CommandExecutor.class, method = "executeCommand")
    public static final class ExecuteCommandPatch {
        @SpirePrefixPatch
        public static SpireReturn<Boolean> prefix(String command) {
            if (!isAbandonCommand(command)) {
                return SpireReturn.Continue();
            }
            abandonRun();
            return SpireReturn.Return(true);
        }
    }

    @SpirePatch(clz = CommandExecutor.class, method = "getAvailableCommands")
    public static final class AvailableCommandsPatch {
        @SpirePostfixPatch
        public static ArrayList<String> postfix(ArrayList<String> result) {
            // CommunicationMod normally advertises START as soon as the
            // dungeon stops being addressable. The game's native start-over
            // reset continues for two seconds after that point, so accepting
            // START during the fade can create a run that is immediately
            // destroyed when the old reset finishes.
            if (CardCrawlGame.startOver && CardCrawlGame.screenTimer > 0.0F) {
                result.remove("start");
            }
            if (isInAbandonableRun() && !result.contains(ABANDON)) {
                result.add(ABANDON);
            }
            return result;
        }
    }
}
