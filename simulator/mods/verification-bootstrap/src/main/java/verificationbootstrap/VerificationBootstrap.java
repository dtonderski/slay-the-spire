package verificationbootstrap;

import basemod.BaseMod;
import basemod.interfaces.PostDungeonInitializeSubscriber;
import com.evacipated.cardcrawl.modthespire.lib.SpireInitializer;
import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePostfixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePrefixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpireReturn;
import com.megacrit.cardcrawl.characters.AbstractPlayer;
import com.megacrit.cardcrawl.core.CardCrawlGame;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import communicationmod.CommandExecutor;
import communicationmod.CommunicationMod;
import communicationmod.GameStateConverter;
import communicationmod.InvalidCommandException;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.concurrent.atomic.AtomicReference;

@SpireInitializer
public final class VerificationBootstrap implements PostDungeonInitializeSubscriber {
    private static final AtomicReference<Integer> PENDING_STARTING_HP = new AtomicReference<>();
    private static final AtomicReference<Integer> ACTIVE_STARTING_HP = new AtomicReference<>();
    private static final ThreadLocal<Boolean> DELEGATING_START = new ThreadLocal<Boolean>() {
        @Override
        protected Boolean initialValue() {
            return false;
        }
    };

    public static void initialize() {
        BaseMod.subscribe(new VerificationBootstrap());
    }

    private static boolean isOrdinaryStart(String command) {
        if (command == null) {
            return false;
        }
        String trimmed = command.trim();
        if (trimmed.isEmpty()) {
            return false;
        }
        return "start".equalsIgnoreCase(trimmed.split("\\s+")[0]);
    }

    private static boolean isStartAvailable() {
        return CommandExecutor.isStartCommandAvailable()
            && !(CardCrawlGame.startOver && CardCrawlGame.screenTimer > 0.0F);
    }

    private static void clearPendingIf(int startingHp) {
        PENDING_STARTING_HP.compareAndSet(startingHp, null);
    }

    @Override
    public void receivePostDungeonInitialize() {
        AbstractPlayer player = AbstractDungeon.player;
        Integer startingHp = PENDING_STARTING_HP.get();
        if (player == null || startingHp == null) {
            return;
        }
        if (!PENDING_STARTING_HP.compareAndSet(startingHp, null)) {
            return;
        }

        player.maxHealth = startingHp;
        player.currentHealth = startingHp;
        player.healthBarUpdatedEvent();
        ACTIVE_STARTING_HP.set(startingHp);
        CommunicationMod.mustSendGameState = true;
    }

    @SpirePatch(clz = CommandExecutor.class, method = "executeCommand")
    public static final class ExecuteCommandPatch {
        @SpirePrefixPatch
        public static SpireReturn<Boolean> prefix(String command) throws InvalidCommandException {
            if (isOrdinaryStart(command)) {
                ACTIVE_STARTING_HP.set(null);
                if (!DELEGATING_START.get()) {
                    PENDING_STARTING_HP.set(null);
                }
                return SpireReturn.Continue();
            }
            if (!StartVerifyCommand.matches(command)) {
                return SpireReturn.Continue();
            }
            if (!isStartAvailable()) {
                throw new InvalidCommandException("START_VERIFY is not currently available");
            }

            final StartVerifyCommand parsed;
            try {
                parsed = StartVerifyCommand.parse(command);
            } catch (IllegalArgumentException error) {
                throw new InvalidCommandException(error.getMessage());
            }

            PENDING_STARTING_HP.set(parsed.startingHp);
            DELEGATING_START.set(true);
            try {
                boolean accepted = CommandExecutor.executeCommand(parsed.normalStartCommand());
                if (!accepted) {
                    clearPendingIf(parsed.startingHp);
                }
                return SpireReturn.Return(accepted);
            } catch (InvalidCommandException | RuntimeException error) {
                clearPendingIf(parsed.startingHp);
                throw error;
            } finally {
                DELEGATING_START.set(false);
            }
        }
    }

    @SpirePatch(clz = CommandExecutor.class, method = "getAvailableCommands")
    public static final class AvailableCommandsPatch {
        @SpirePostfixPatch
        public static ArrayList<String> postfix(ArrayList<String> result) {
            if (isStartAvailable() && !result.contains(StartVerifyCommand.NAME)) {
                result.add(StartVerifyCommand.NAME);
            } else if (!isStartAvailable()) {
                result.remove(StartVerifyCommand.NAME);
            }
            return result;
        }
    }

    @SpirePatch(clz = GameStateConverter.class, method = "getGameState")
    public static final class GameStatePatch {
        @SpirePostfixPatch
        public static HashMap<String, Object> postfix(HashMap<String, Object> result) {
            Integer startingHp = ACTIVE_STARTING_HP.get();
            if (startingHp != null && AbstractDungeon.player != null) {
                result.put("verification_starting_hp", startingHp);
            }
            return result;
        }
    }
}
