package abandonruncontrol;

import com.badlogic.gdx.Gdx;
import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePostfixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePrefixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpireReturn;
import com.megacrit.cardcrawl.core.CardCrawlGame;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.saveAndContinue.SaveAndContinue;
import com.megacrit.cardcrawl.screens.mainMenu.MainMenuScreen;
import communicationmod.CommandExecutor;
import java.util.ArrayList;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

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
        runOnGameThread(new Runnable() {
            @Override
            public void run() {
                abandonRunNow();
            }
        });
    }

    private static void runOnGameThread(Runnable runnable) {
        CountDownLatch done = new CountDownLatch(1);
        Gdx.app.postRunnable(new Runnable() {
            @Override
            public void run() {
                try {
                    runnable.run();
                } finally {
                    done.countDown();
                }
            }
        });
        try {
            done.await(2, TimeUnit.SECONDS);
        } catch (InterruptedException ignored) {
            Thread.currentThread().interrupt();
        }
    }

    private static void abandonRunNow() {
        if (!isInAbandonableRun()) {
            forceStartOver();
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

        forceStartOver();
    }

    private static void forceStartOver() {
        try {
            AbstractDungeon.screen = AbstractDungeon.CurrentScreen.NONE;
        } catch (RuntimeException ignored) {
        }

        try {
            AbstractDungeon.reset();
        } catch (RuntimeException ignored) {
        }

        try {
            CardCrawlGame.mainMenuScreen = new MainMenuScreen();
            if (CardCrawlGame.mainMenuScreen.bg != null) {
                CardCrawlGame.mainMenuScreen.bg.slideDownInstantly();
            }
        } catch (RuntimeException ignored) {
        }

        CardCrawlGame.mode = CardCrawlGame.GameMode.CHAR_SELECT;
        CardCrawlGame.startOver = false;
        CardCrawlGame.fadeIn(0.25F);
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
            if (isInAbandonableRun() && !result.contains(ABANDON)) {
                result.add(ABANDON);
            }
            return result;
        }
    }
}
