package communicationmod;

import com.megacrit.cardcrawl.core.CardCrawlGame;
import com.megacrit.cardcrawl.core.Settings;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.rewards.RewardItem;
import com.megacrit.cardcrawl.vfx.AbstractGameEffect;

import java.io.BufferedWriter;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardOpenOption;
/**
 * Bounded, opt-in diagnostics for the Recall/Dig command-to-effect window.
 * This class never participates in a readiness predicate and never changes a
 * game collection.  It is deliberately string/class-name based so enabling
 * diagnostics does not initialize campfire option classes.
 */
public final class CampfireDiagnostics {
    public static final String PATH_PROPERTY = "communicationmod.diagnostics.path";
    public static final String CAP_PROPERTY = "communicationmod.diagnostics.maxRecords";
    public static final String TAIL_PROPERTY = "communicationmod.diagnostics.tailUpdates";
    public static final String MIN_COMMAND_PROPERTY = "communicationmod.diagnostics.minCommandExecutionSeq";
    private static final int DEFAULT_CAP = 512;
    private static final int DEFAULT_TAIL_UPDATES = 32;

    private static BufferedWriter writer;
    private static Path outputPath;
    private static int recordCount;
    private static int cap;
    private static boolean initialized;
    private static boolean active;
    private static boolean passiveTail;
    private static int passiveTailUpdates;
    private static boolean relevantEffectObserved;
    private static String optionClass;
    private static String commandId;
    private static long commandExecutionSeq;
    private static long commandSettlementSeq;
    private static long commandUpdateSeq;
    private static boolean keyRuby;
    private static boolean keyEmerald;
    private static boolean keySapphire;

    private CampfireDiagnostics() { }

    /** Called at the command's campfire option entry, before useOption(). */
    public static void commandEntry(String selectedClass) {
        if (selectedClass == null) {
            return;
        }
        String simple = selectedClass;
        int dollar = simple.lastIndexOf('$');
        if (dollar >= 0) simple = simple.substring(dollar + 1);
        if (!simple.endsWith("RecallOption") && !simple.endsWith("DigOption") && !simple.endsWith("SmithOption")) {
            return;
        }
        ensureInitialized();
        if (!enabled()) return;
        active = true;
        passiveTail = false;
        passiveTailUpdates = parseTail(System.getProperty(TAIL_PROPERTY));
        relevantEffectObserved = false;
        optionClass = selectedClass;
        commandId = GameStateListener.getActiveCommandId();
        commandExecutionSeq = GameStateListener.getCommandExecutionSeq();
        commandSettlementSeq = GameStateListener.getCommandSettlementSeq();
        commandUpdateSeq = GameStateListener.getGameUpdateSeq();
        writeSnapshot("command_entry", null, null);
    }

    /** Starts a bounded card-reward acquire window for FastCardObtainEffect. */
    public static void cardRewardCommandEntry() {
        if (active) return;
        ensureInitialized();
        if (!enabled()) return;
        active = true;
        // The card command window records pre-publication effects too, but its
        // bounded tail starts only at the first published boundary.
        passiveTail = false;
        passiveTailUpdates = parseTail(System.getProperty(TAIL_PROPERTY));
        relevantEffectObserved = false;
        optionClass = "CardReward";
        commandId = GameStateListener.getActiveCommandId();
        commandExecutionSeq = GameStateListener.getCommandExecutionSeq();
        commandSettlementSeq = GameStateListener.getCommandSettlementSeq();
        commandUpdateSeq = GameStateListener.getGameUpdateSeq();
        writeSnapshot("card_reward_command_entry", null, null);
    }

    /** Target effect update prefix. No game method is called here. */
    public static void effectUpdateEntered(String effectClass, AbstractGameEffect effect) {
        if (!active || !enabled()) return;
        observeRelevantEffect(effectClass, effect);
        keyRuby = Settings.hasRubyKey;
        keyEmerald = Settings.hasEmeraldKey;
        keySapphire = Settings.hasSapphireKey;
        writeSnapshot("effect_update_enter", effectClass, effect);
    }

    /** Target effect update postfix. Captures key grant without forcing it. */
    public static void effectUpdateExited(String effectClass, AbstractGameEffect effect) {
        if (!active || !enabled()) return;
        writeSnapshot("effect_update_exit", effectClass, effect);
        if (keyRuby != Settings.hasRubyKey || keyEmerald != Settings.hasEmeraldKey
                || keySapphire != Settings.hasSapphireKey) {
            writeSnapshot("key_grant", effectClass, effect);
        }
    }

    /** Observation seam for the first half of the paired update callback. */
    public static void gameUpdate(String seam) {
        if (!active || !enabled()) return;
        writeSnapshot(seam, null, null);
    }

    /** Called from the existing PostDungeonUpdate seam. */
    public static void dungeonUpdate(String seam) {
        if (!active || !enabled()) return;
        writeSnapshot(seam, null, null);
        if (!passiveTail || !"dungeon_update_post".equals(seam)) return;

        try {
            observeRelevantEffects();
            if (relevantEffectObserved && !hasPendingRelevantEffect()) {
                // The natural effect hook/collection observation has shown the
                // lifecycle ending. Keep the completion row above, then close.
                active = false;
                passiveTail = false;
            } else if (--passiveTailUpdates <= 0) {
                writeTruncation("tail_updates");
            }
        } catch (RuntimeException ignored) {
            // Collection traversal is diagnostic work and must not escape into
            // the game's update callback.
            disableDiagnostics();
        }
    }

    /** Called only after the existing listener has decided to publish a boundary. */
    public static void statePublished(String stage) {
        if (!active || !enabled()) return;
        commandSettlementSeq = GameStateListener.getCommandSettlementSeq();
        commandUpdateSeq = GameStateListener.getGameUpdateSeq();
        writeSnapshot(stage, null, null);
        if (!active || !enabled()) return;
        // Publication can precede a child effect (Recall's key or a card
        // obtain) becoming visible. Continue with passive observations rather
        // than making the first publication terminate the diagnostic window.
        passiveTail = true;
        passiveTailUpdates = parseTail(System.getProperty(TAIL_PROPERTY));
    }

    private static boolean enabled() {
        return outputPath != null && writer != null && recordCount < cap;
    }

    private static void ensureInitialized() {
        if (initialized) return;
        if (GameStateListener.getCommandExecutionSeq() < Long.getLong(MIN_COMMAND_PROPERTY, 0L)) return;
        initialized = true;
        String configured = System.getProperty(PATH_PROPERTY);
        if (configured == null || configured.trim().isEmpty()) return;
        cap = parseCap(System.getProperty(CAP_PROPERTY));
        try {
            Path requested = Paths.get(configured.trim());
            if (!requested.isAbsolute()) {
                throw new IllegalArgumentException("diagnostic output path must be absolute");
            }
            outputPath = collisionFreePath(requested);
            if (outputPath.getParent() != null) Files.createDirectories(outputPath.getParent());
            writer = Files.newBufferedWriter(outputPath, StandardCharsets.UTF_8,
                    StandardOpenOption.CREATE_NEW, StandardOpenOption.WRITE);
        } catch (IOException | RuntimeException ignored) {
            // Diagnostics must be safe when unavailable and must never affect protocol output.
            writer = null;
            outputPath = null;
        }
    }

    private static int parseCap(String value) {
        if (value == null) return DEFAULT_CAP;
        try {
            int parsed = Integer.parseInt(value);
            return parsed > 0 ? parsed : DEFAULT_CAP;
        } catch (NumberFormatException ignored) {
            return DEFAULT_CAP;
        }
    }

    private static int parseTail(String value) {
        if (value == null) return DEFAULT_TAIL_UPDATES;
        try {
            int parsed = Integer.parseInt(value);
            return parsed > 0 ? parsed : DEFAULT_TAIL_UPDATES;
        } catch (NumberFormatException ignored) {
            return DEFAULT_TAIL_UPDATES;
        }
    }

    private static Path collisionFreePath(Path requested) {
        if (!Files.exists(requested)) return requested;
        String text = requested.toString();
        for (int suffix = 1; suffix < 10000; suffix++) {
            Path candidate = Paths.get(text + "." + suffix);
            if (!Files.exists(candidate)) return candidate;
        }
        throw new IllegalStateException("too many diagnostic output collisions");
    }

    private static void writeSnapshot(String stage, String effectClass, AbstractGameEffect effect) {
        if (!enabled()) return;
        try {
            if (!reserveRecord()) return;
            // Headless/static validation can load the mod before localization
            // and AbstractDungeon are initialized. Keep diagnostics inert there
            // rather than becoming the initializer of gameplay state.
            if (CardCrawlGame.languagePack == null) {
                writeMinimalSnapshot(stage, effectClass, effect);
                return;
            }
            StringBuilder json = new StringBuilder(768);
            json.append('{');
            field(json, "stage", stage);
            field(json, "option_class", optionClass);
            field(json, "command_id", commandId);
            number(json, "command_execution_seq", commandExecutionSeq);
            number(json, "current_command_execution_seq", GameStateListener.getCommandExecutionSeq());
            field(json, "active_command_id", GameStateListener.getActiveCommandId());
            json.append(",\"campfire_hidden\":").append(com.megacrit.cardcrawl.rooms.CampfireUI.hidden);
            number(json, "command_settlement_seq", commandSettlementSeq);
            number(json, "game_update_seq", GameStateListener.getGameUpdateSeq());
            number(json, "dungeon_update_seq", GameStateListener.getDungeonUpdateSeq());
            number(json, "record_index", recordCount);
            field(json, "effect_class", effectClass);
            appendEffectFields(json, effect);
            json.append(",\"listener_blocked\":").append(GameStateListener.isStateUpdateBlocked());
            json.append(",\"wait_one_update\":").append(GameStateListener.isWaitingOneUpdate());
            field(json, "state_publish_stage", GameStateListener.getBoundaryKind());
            field(json, "screen", currentScreen());
            json.append(",\"is_screen_up\":").append(AbstractDungeon.isScreenUp);
            field(json, "room_phase", currentRoomPhase());
            json.append(",\"keys\":{");
            json.append("\"ruby\":").append(Settings.hasRubyKey);
            json.append(",\"emerald\":").append(Settings.hasEmeraldKey);
            json.append(",\"sapphire\":").append(Settings.hasSapphireKey).append('}');
            json.append(",\"reward_classes\":[");
            appendRewardClasses(json);
            json.append(']');
            json.append(",\"effect_collections\":{");
            appendEffects(json, "effect_list", AbstractDungeon.effectList);
            json.append(',');
            appendEffects(json, "effects_queue", AbstractDungeon.effectsQueue);
            json.append(',');
            appendEffects(json, "top_level_effects", AbstractDungeon.topLevelEffects);
            json.append(',');
            appendEffects(json, "top_level_effects_queue", AbstractDungeon.topLevelEffectsQueue);
            json.append("}}");
            appendRecord(json.toString());
        } catch (RuntimeException ignored) {
            // A malformed transient target object must not escape the observer
            // into the game's update loop or alter gameplay.
            disableDiagnostics();
        }
    }

    private static void writeMinimalSnapshot(String stage, String effectClass, AbstractGameEffect effect) {
        if (!reserveRecord()) return;
        StringBuilder json = new StringBuilder(192);
        json.append('{');
        field(json, "stage", stage);
        field(json, "option_class", optionClass);
        field(json, "command_id", commandId);
        field(json, "effect_class", effectClass);
        number(json, "record_index", recordCount);
        appendEffectFields(json, effect);
        json.append('}');
        appendRecord(json.toString());
    }

    private static void appendEffectFields(StringBuilder json, AbstractGameEffect effect) {
        if (effect == null) return;
        number(json, "effect_identity", System.identityHashCode(effect));
        jsonField(json, "duration", Float.toString(effect.duration));
        jsonField(json, "starting_duration", Float.toString(effect.startingDuration));
        json.append(",\"is_done\":").append(effect.isDone);
    }

    private static void appendRecord(String json) {
        try {
            writer.write(json);
            writer.newLine();
            writer.flush();
            recordCount++;
        } catch (IOException | RuntimeException ignored) {
            disableDiagnostics();
        }
    }

    private static boolean reserveRecord() {
        if (recordCount < cap - 1) return true;
        writeTruncation("max_records");
        return false;
    }

    private static void writeTruncation(String reason) {
        if (writer == null || recordCount >= cap) {
            disableDiagnostics();
            return;
        }
        StringBuilder json = new StringBuilder(128);
        json.append('{');
        field(json, "stage", "truncated");
        field(json, "option_class", optionClass);
        number(json, "record_index", recordCount);
        json.append(",\"truncated\":true");
        field(json, "reason", reason);
        json.append('}');
        appendRecord(json.toString());
        active = false;
        passiveTail = false;
    }

    private static String currentScreen() {
        return AbstractDungeon.screen == null ? null : AbstractDungeon.screen.name();
    }

    private static String currentRoomPhase() {
        if (AbstractDungeon.currMapNode == null || AbstractDungeon.getCurrRoom() == null
                || AbstractDungeon.getCurrRoom().phase == null) {
            return null;
        }
        return AbstractDungeon.getCurrRoom().phase.name();
    }

    private static void appendRewardClasses(StringBuilder json) {
        if (AbstractDungeon.combatRewardScreen == null
                || AbstractDungeon.combatRewardScreen.rewards == null) return;
        boolean first = true;
        for (RewardItem reward : AbstractDungeon.combatRewardScreen.rewards) {
            if (!first) json.append(',');
            fieldValue(json, reward == null ? null : reward.getClass().getName());
            first = false;
        }
    }

    private static void appendEffects(StringBuilder json, String name, Iterable<AbstractGameEffect> effects) {
        fieldValue(json, name);
        json.append(':').append('[');
        boolean first = true;
        if (effects != null) {
            for (AbstractGameEffect effect : effects) {
                if (!first) json.append(',');
                json.append('{');
                field(json, "class", effect == null ? null : effect.getClass().getName());
                appendEffectFields(json, effect);
                json.append('}');
                first = false;
            }
        }
        json.append(']');
    }

    private static void observeRelevantEffect(String effectClass, AbstractGameEffect effect) {
        if (isRelevantEffect(effectClass) || (effect != null && isRelevantEffect(effect.getClass().getName()))) {
            relevantEffectObserved = true;
        }
    }

    private static void observeRelevantEffects() {
        if (CardCrawlGame.languagePack == null) return;
        observeRelevantEffects(AbstractDungeon.effectList);
        observeRelevantEffects(AbstractDungeon.effectsQueue);
        observeRelevantEffects(AbstractDungeon.topLevelEffects);
        observeRelevantEffects(AbstractDungeon.topLevelEffectsQueue);
    }

    private static void observeRelevantEffects(Iterable<AbstractGameEffect> effects) {
        if (effects == null) return;
        for (AbstractGameEffect effect : effects) {
            if (effect != null && isRelevantEffect(effect.getClass().getName())) {
                relevantEffectObserved = true;
            }
        }
    }

    private static boolean hasPendingRelevantEffect() {
        if (CardCrawlGame.languagePack == null) return false;
        return hasPendingRelevantEffect(AbstractDungeon.effectList)
                || hasPendingRelevantEffect(AbstractDungeon.effectsQueue)
                || hasPendingRelevantEffect(AbstractDungeon.topLevelEffects)
                || hasPendingRelevantEffect(AbstractDungeon.topLevelEffectsQueue);
    }

    private static boolean hasPendingRelevantEffect(Iterable<AbstractGameEffect> effects) {
        if (effects == null) return false;
        for (AbstractGameEffect effect : effects) {
            if (effect != null && !effect.isDone && isRelevantEffect(effect.getClass().getName())) return true;
        }
        return false;
    }

    private static boolean isRelevantEffect(String effectClass) {
        if (effectClass == null) return false;
        if ("CardReward".equals(optionClass)) return effectClass.endsWith("FastCardObtainEffect");
        if (optionClass != null && optionClass.endsWith("RecallOption")) {
            return effectClass.endsWith("CampfireRecallEffect") || effectClass.endsWith("ObtainKeyEffect");
        }
        if (optionClass != null && optionClass.endsWith("DigOption")) {
            return effectClass.endsWith("CampfireDigEffect");
        }
        if (optionClass != null && optionClass.endsWith("SmithOption")) {
            return effectClass.endsWith("CampfireSmithEffect");
        }
        return false;
    }

    private static void field(StringBuilder json, String key, String value) {
        separator(json);
        fieldValue(json, key);
        json.append(':');
        fieldValue(json, value);
    }

    private static void number(StringBuilder json, String key, long value) {
        separator(json);
        fieldValue(json, key);
        json.append(':').append(value);
    }

    private static void jsonField(StringBuilder json, String key, String value) {
        separator(json);
        fieldValue(json, key);
        json.append(':').append(value);
    }

    private static void separator(StringBuilder json) {
        int length = json.length();
        if (length > 0 && json.charAt(length - 1) != '{' && json.charAt(length - 1) != '[') {
            json.append(',');
        }
    }

    private static void fieldValue(StringBuilder json, String value) {
        if (value == null) {
            json.append("null");
            return;
        }
        json.append('"');
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            if (c == '\\' || c == '"') json.append('\\');
            if (c == '\n') json.append('n');
            else if (c == '\r') json.append('r');
            else if (c == '\t') json.append('t');
            else json.append(c);
        }
        json.append('"');
    }

    private static void closeWriter() {
        if (writer != null) {
            try { writer.close(); } catch (IOException ignored) { }
        }
        writer = null;
        outputPath = null;
    }

    private static void disableDiagnostics() {
        closeWriter();
        active = false;
        passiveTail = false;
    }

    // Package-private test hooks; they do not access or mutate game state.
    static void resetForTests() {
        closeWriter();
        initialized = false;
        recordCount = 0;
        cap = 0;
        active = false;
        passiveTail = false;
        passiveTailUpdates = 0;
        relevantEffectObserved = false;
        optionClass = null;
        commandId = null;
    }

    static String outputPathForTests() {
        return outputPath == null ? null : outputPath.toString();
    }

    static int recordCountForTests() { return recordCount; }
}
