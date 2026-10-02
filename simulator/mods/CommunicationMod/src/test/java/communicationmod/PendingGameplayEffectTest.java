package communicationmod;

import com.badlogic.gdx.Files;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.Graphics;
import com.badlogic.gdx.Input;
import com.badlogic.gdx.files.FileHandle;
import com.badlogic.gdx.graphics.Texture;
import com.badlogic.gdx.graphics.TextureData;
import com.badlogic.gdx.graphics.g2d.BitmapFont;
import com.badlogic.gdx.graphics.g2d.GlyphLayout;
import com.badlogic.gdx.graphics.g2d.TextureRegion;
import com.badlogic.gdx.utils.Array;
import com.google.gson.Gson;
import com.google.gson.JsonObject;
import com.megacrit.cardcrawl.actions.GameActionManager;
import com.megacrit.cardcrawl.cards.CardGroup;
import com.megacrit.cardcrawl.cards.Soul;
import com.megacrit.cardcrawl.cards.SoulGroup;
import com.megacrit.cardcrawl.characters.AbstractPlayer;
import com.megacrit.cardcrawl.characters.Ironclad;
import com.megacrit.cardcrawl.core.CardCrawlGame;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.dungeons.TheCity;
import com.megacrit.cardcrawl.events.shrines.GremlinMatchGame;
import com.megacrit.cardcrawl.helpers.Prefs;
import com.megacrit.cardcrawl.monsters.MonsterGroup;
import com.megacrit.cardcrawl.potions.AbstractPotion;
import com.megacrit.cardcrawl.potions.PotionSlot;
import com.megacrit.cardcrawl.rooms.EventRoom;
import basemod.ReflectionHacks;
import com.megacrit.cardcrawl.localization.CharacterStrings;
import com.megacrit.cardcrawl.localization.EventStrings;
import com.megacrit.cardcrawl.localization.LocalizedStrings;
import com.megacrit.cardcrawl.localization.TutorialStrings;
import com.megacrit.cardcrawl.localization.UIStrings;
import com.megacrit.cardcrawl.map.MapRoomNode;
import com.megacrit.cardcrawl.rooms.AbstractRoom;
import com.megacrit.cardcrawl.rooms.RestRoom;
import com.megacrit.cardcrawl.vfx.AbstractGameEffect;
import com.megacrit.cardcrawl.vfx.ObtainKeyEffect;
import com.megacrit.cardcrawl.vfx.ObtainPotionEffect;
import com.megacrit.cardcrawl.vfx.FastCardObtainEffect;
import com.megacrit.cardcrawl.vfx.campfire.CampfireRecallEffect;
import com.megacrit.cardcrawl.vfx.campfire.CampfireDigEffect;
import com.megacrit.cardcrawl.vfx.campfire.CampfireSmithEffect;
import com.megacrit.cardcrawl.vfx.campfire.CampfireTokeEffect;
import com.megacrit.cardcrawl.vfx.cardManip.ShowCardAndObtainEffect;
import com.megacrit.cardcrawl.vfx.cardManip.ExhaustCardEffect;
import org.junit.After;
import org.junit.Before;
import org.junit.Test;
import sun.misc.Unsafe;

import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertSame;
import static org.junit.Assert.assertTrue;

public class PendingGameplayEffectTest {
    private TestStateSnapshot statics;

    @Before
    public void captureStatics() {
        statics = TestStateSnapshot.of(GameStateListener.class, CardCrawlGame.class, Gdx.class);
    }

    @After
    public void restoreStatics() {
        statics.restore();
    }

    private static final Unsafe UNSAFE = getUnsafe();

    private static Unsafe getUnsafe() {
        try {
            Field field = Unsafe.class.getDeclaredField("theUnsafe");
            field.setAccessible(true);
            return (Unsafe) field.get(null);
        } catch (ReflectiveOperationException exception) {
            throw new AssertionError(exception);
        }
    }

    @SuppressWarnings("unchecked")
    private static <T> T allocate(Class<T> type) throws InstantiationException {
        return (T) UNSAFE.allocateInstance(type);
    }

    private static final class DecorativeEffect extends AbstractGameEffect {
        @Override
        public void render(com.badlogic.gdx.graphics.g2d.SpriteBatch spriteBatch) {
        }

        @Override
        public void dispose() {
        }
    }

    @Test
    public void productionReadinessAndSerializationCoverBothGameplayEffectQueues() throws Exception {
        Path configPath = Paths.get("info.displayconfig");
        boolean initialConfigExists = java.nio.file.Files.exists(configPath);
        byte[] initialConfig = initialConfigExists
                ? java.nio.file.Files.readAllBytes(configPath) : null;
        FixtureSnapshot baseline = null;
        try {
            // Exercise absence even when a developer already has a config;
            // the outer finally restores their original bytes.
            java.nio.file.Files.deleteIfExists(configPath);
            baseline = runProductionScenario();
            baseline.assertGlobalsRestored();
            assertFalse(java.nio.file.Files.exists(configPath));

            byte[] existingConfig = "sentinel-display-config".getBytes("UTF-8");
            java.nio.file.Files.write(configPath, existingConfig);
            FixtureSnapshot secondRun = runProductionScenario();
            secondRun.assertGlobalsRestored();
            baseline.assertGlobalsRestored();
            assertArrayEquals(existingConfig, java.nio.file.Files.readAllBytes(configPath));
        } finally {
            try {
                if (baseline != null) {
                    baseline.restoreGlobals();
                }
            } finally {
                if (initialConfigExists) {
                    java.nio.file.Files.write(configPath, initialConfig);
                } else {
                    java.nio.file.Files.deleteIfExists(configPath);
                }
            }
        }
    }

    private static FixtureSnapshot runProductionScenario() throws Exception {
        TestStateSnapshot listenerStatics = TestStateSnapshot.of(GameStateListener.class);
        FixtureSnapshot snapshot = FixtureSnapshot.capture();
        try {
            installHeadlessGameDependencies(snapshot);
            snapshot.captureDungeon();
            configureDungeonFixture();

            for (Class<? extends AbstractGameEffect> effectType : new Class[]{
                    ObtainKeyEffect.class, ObtainPotionEffect.class,
                    ShowCardAndObtainEffect.class, FastCardObtainEffect.class,
                    ExhaustCardEffect.class, CampfireRecallEffect.class, CampfireDigEffect.class
            }) {
                GameStateListener.resetStateVariables();
                AbstractDungeon.effectList.clear();
                AbstractDungeon.effectsQueue.clear();
                AbstractGameEffect effect = allocate(effectType);
                effect.isDone = false;
                AbstractDungeon.effectsQueue.add(effect);

                // These calls are the production listener path. The queued
                // dungeon list must prevent a ready boundary before drain.
                assertFalse(GameStateListener.checkForDungeonStateChange());
                assertFalse(GameStateListener.checkForDungeonStateChange());
                JsonObject queued = stateJson();
                assertFalse(queued.get("ready_for_command").getAsBoolean());
                assertEquals(0, queued.get("effects_size").getAsInt());
                assertEquals(1, queued.get("queued_effects_size").getAsInt());

                AbstractDungeon.effectsQueue.remove(effect);
                AbstractDungeon.effectList.add(effect);
                assertFalse(GameStateListener.checkForDungeonStateChange());
                JsonObject active = stateJson();
                assertFalse(active.get("ready_for_command").getAsBoolean());
                assertEquals(1, active.get("effects_size").getAsInt());
                assertEquals(0, active.get("queued_effects_size").getAsInt());

                effect.isDone = true;
                assertFalse(GameStateListener.checkForDungeonStateChange());
                assertTrue(GameStateListener.checkForDungeonStateChange());
                JsonObject completed = stateJson();
                assertTrue(completed.get("ready_for_command").getAsBoolean());
                assertEquals(0, completed.get("effects_size").getAsInt());
                assertEquals(0, completed.get("queued_effects_size").getAsInt());
            }

            GameStateListener.resetStateVariables();
            AbstractDungeon.effectList.clear();
            AbstractDungeon.effectsQueue.clear();
            Soul soul = allocate(Soul.class);
            soul.isReadyForReuse = false;
            SoulGroup souls = allocate(SoulGroup.class);
            ReflectionHacks.setPrivate(souls, SoulGroup.class, "souls",
                    new ArrayList<Soul>(Arrays.asList(soul)));
            AbstractDungeon.getCurrRoom().souls = souls;
            assertTrue(SoulGroup.isActive());
            assertFalse(GameStateListener.checkForDungeonStateChange());
            assertFalse(GameStateListener.checkForDungeonStateChange());
            assertFalse(stateJson().get("ready_for_command").getAsBoolean());
            assertFalse(soul.isReadyForReuse);

            // Only the game's own Soul.update finishes clearPowers/applyPowers.
            // Model completion here; the listener must not advance the soul.
            soul.isReadyForReuse = true;
            assertFalse(GameStateListener.checkForDungeonStateChange());
            assertTrue(GameStateListener.checkForDungeonStateChange());
            assertTrue(stateJson().get("ready_for_command").getAsBoolean());

            // Death retains priority even if a travelling card remains.
            soul.isReadyForReuse = false;
            GameStateListener.resetStateVariables();
            AbstractDungeon.CurrentScreen savedScreen = AbstractDungeon.screen;
            AbstractDungeon.screen = AbstractDungeon.CurrentScreen.DEATH;
            assertTrue(GameStateListener.checkForDungeonStateChange());
            assertFalse(soul.isReadyForReuse);
            AbstractDungeon.screen = savedScreen;
            AbstractDungeon.getCurrRoom().souls = null;

            GameStateListener.resetStateVariables();
            AbstractDungeon.effectsQueue.add(new DecorativeEffect());
            assertFalse(GameStateListener.checkForDungeonStateChange());
            assertTrue(GameStateListener.checkForDungeonStateChange());
            JsonObject decorative = stateJson();
            assertTrue(decorative.get("ready_for_command").getAsBoolean());
            assertEquals(0, decorative.get("queued_effects_size").getAsInt());
        } finally {
            try {
                snapshot.restoreGlobals();
            } finally {
                try {
                    snapshot.restoreConfigFile();
                } finally {
                    GameStateListener.resetStateVariables();
                    listenerStatics.restore();
                }
            }
        }
        return snapshot;
    }

    @Test
    public void multipleQueuedPotionObtainsWaitForEveryNaturalUpdate() throws Exception {
        FixtureSnapshot fixture = FixtureSnapshot.capture();
        try {
            installHeadlessGameDependencies(fixture);
            fixture.captureDungeon();
            configureDungeonFixture();
            GameStateListener.resetStateVariables();
            ObtainPotionEffect first = allocate(ObtainPotionEffect.class);
            ObtainPotionEffect second = allocate(ObtainPotionEffect.class);
            AbstractDungeon.effectsQueue.add(first);
            AbstractDungeon.effectsQueue.add(second);
            assertFalse(GameStateListener.checkForDungeonStateChange());
            first.isDone = true; // Test-only stand-in for the game's own update.
            assertFalse(GameStateListener.checkForDungeonStateChange());
            AbstractDungeon.effectsQueue.remove(first);
            AbstractDungeon.effectList.add(second);
            assertFalse(GameStateListener.checkForDungeonStateChange());
            second.isDone = true;
            assertFalse(GameStateListener.checkForDungeonStateChange());
            assertTrue(GameStateListener.checkForDungeonStateChange());
            assertEquals(0, GameStateListener.getQueuedEffectQueueSize());
            assertEquals(0, GameStateListener.getEffectQueueSize());
        } finally {
            try { fixture.restoreGlobals(); } finally { fixture.restoreConfigFile(); }
        }
    }

    private static final class FixtureSnapshot {
        private static FixtureSnapshot capture() {
            return new FixtureSnapshot();
        }

        private final CardCrawlGame.GameMode mode = CardCrawlGame.mode;
        private final LocalizedStrings languagePack = CardCrawlGame.languagePack;
        private AbstractDungeon dungeon;
        private MapRoomNode currMapNode;
        private AbstractPlayer player;
        private GameActionManager actionManager;
        private ArrayList<AbstractGameEffect> effects;
        private ArrayList<AbstractGameEffect> effectsQueue;
        private ArrayList<AbstractGameEffect> topLevelEffects;
        private ArrayList<AbstractGameEffect> topLevelEffectsQueue;
        private ArrayList<ArrayList<MapRoomNode>> map;
        private com.megacrit.cardcrawl.screens.select.GridCardSelectScreen gridSelectScreen;
        private boolean screenUp;
        private AbstractDungeon.CurrentScreen screen;
        private boolean dungeonCaptured;
        private final Files files = Gdx.files;
        private final Graphics graphics = Gdx.graphics;
        private final Input input = Gdx.input;
        private final Prefs gamePref = com.megacrit.cardcrawl.core.Settings.gamePref;
        private final Prefs soundPref = com.megacrit.cardcrawl.core.Settings.soundPref;
        private final Prefs achievementPref = com.megacrit.cardcrawl.unlock.UnlockTracker.achievementPref;
        private final GlyphLayout layout = com.megacrit.cardcrawl.helpers.FontHelper.layout;
        private final Map<Field, BitmapFont> fonts = captureFonts();
        private final Map<Field, Texture> textures = captureTextures();
        private final Path configPath = Paths.get("info.displayconfig");
        private final boolean configExists = java.nio.file.Files.exists(configPath);
        private final byte[] configBytes = readConfig(configPath, configExists);

        private void captureDungeon() {
            dungeon = CardCrawlGame.dungeon;
            currMapNode = AbstractDungeon.currMapNode;
            player = AbstractDungeon.player;
            actionManager = AbstractDungeon.actionManager;
            effects = AbstractDungeon.effectList;
            effectsQueue = AbstractDungeon.effectsQueue;
            topLevelEffects = AbstractDungeon.topLevelEffects;
            topLevelEffectsQueue = AbstractDungeon.topLevelEffectsQueue;
            map = AbstractDungeon.map;
            gridSelectScreen = AbstractDungeon.gridSelectScreen;
            screenUp = AbstractDungeon.isScreenUp;
            screen = AbstractDungeon.screen;
            dungeonCaptured = true;
        }

        private static Map<Field, BitmapFont> captureFonts() {
            try {
                Map<Field, BitmapFont> result = new HashMap<>();
                for (Field field : com.megacrit.cardcrawl.helpers.FontHelper.class.getFields()) {
                    if (field.getType() == BitmapFont.class) {
                        result.put(field, (BitmapFont) field.get(null));
                    }
                }
                return result;
            } catch (ReflectiveOperationException exception) {
                throw new AssertionError(exception);
            }
        }

        private static Map<Field, Texture> captureTextures() {
            try {
                Map<Field, Texture> result = new HashMap<>();
                Class<?> imageMaster = com.megacrit.cardcrawl.helpers.ImageMaster.class;
                for (Field field : imageMaster.getDeclaredFields()) {
                    if (Modifier.isStatic(field.getModifiers()) && field.getType() == Texture.class) {
                        field.setAccessible(true);
                        result.put(field, (Texture) field.get(null));
                    }
                }
                return result;
            } catch (ReflectiveOperationException exception) {
                throw new AssertionError(exception);
            }
        }

        private static byte[] readConfig(Path path, boolean exists) {
            try {
                return exists ? java.nio.file.Files.readAllBytes(path) : null;
            } catch (java.io.IOException exception) {
                throw new AssertionError(exception);
            }
        }

        private void restoreGlobals() throws Exception {
            CardCrawlGame.mode = mode;
            CardCrawlGame.languagePack = languagePack;
            if (dungeonCaptured) {
                CardCrawlGame.dungeon = dungeon;
                AbstractDungeon.currMapNode = currMapNode;
                AbstractDungeon.player = player;
                AbstractDungeon.actionManager = actionManager;
                AbstractDungeon.effectList = effects;
                AbstractDungeon.effectsQueue = effectsQueue;
                AbstractDungeon.topLevelEffects = topLevelEffects;
                AbstractDungeon.topLevelEffectsQueue = topLevelEffectsQueue;
                AbstractDungeon.map = map;
                AbstractDungeon.gridSelectScreen = gridSelectScreen;
                AbstractDungeon.isScreenUp = screenUp;
                AbstractDungeon.screen = screen;
            }
            Gdx.files = files;
            Gdx.graphics = graphics;
            Gdx.input = input;
            com.megacrit.cardcrawl.core.Settings.gamePref = gamePref;
            com.megacrit.cardcrawl.core.Settings.soundPref = soundPref;
            com.megacrit.cardcrawl.unlock.UnlockTracker.achievementPref = achievementPref;
            com.megacrit.cardcrawl.helpers.FontHelper.layout = layout;
            for (Map.Entry<Field, BitmapFont> entry : fonts.entrySet()) {
                entry.getKey().set(null, entry.getValue());
            }
            for (Map.Entry<Field, Texture> entry : textures.entrySet()) {
                entry.getKey().set(null, entry.getValue());
            }
        }

        private void restoreConfigFile() throws Exception {
            if (configExists) {
                java.nio.file.Files.write(configPath, configBytes);
            } else {
                java.nio.file.Files.deleteIfExists(configPath);
            }
        }

        private void assertGlobalsRestored() throws Exception {
            assertSame(mode, CardCrawlGame.mode);
            assertSame(languagePack, CardCrawlGame.languagePack);
            assertTrue(dungeonCaptured);
            assertSame(dungeon, CardCrawlGame.dungeon);
            assertSame(currMapNode, AbstractDungeon.currMapNode);
            assertSame(player, AbstractDungeon.player);
            assertSame(actionManager, AbstractDungeon.actionManager);
            assertSame(effects, AbstractDungeon.effectList);
            assertSame(effectsQueue, AbstractDungeon.effectsQueue);
            assertSame(topLevelEffects, AbstractDungeon.topLevelEffects);
            assertSame(topLevelEffectsQueue, AbstractDungeon.topLevelEffectsQueue);
            assertSame(map, AbstractDungeon.map);
            assertSame(gridSelectScreen, AbstractDungeon.gridSelectScreen);
            assertEquals(screenUp, AbstractDungeon.isScreenUp);
            assertSame(screen, AbstractDungeon.screen);
            assertSame(files, Gdx.files);
            assertSame(graphics, Gdx.graphics);
            assertSame(input, Gdx.input);
            assertSame(gamePref, com.megacrit.cardcrawl.core.Settings.gamePref);
            assertSame(soundPref, com.megacrit.cardcrawl.core.Settings.soundPref);
            assertSame(achievementPref, com.megacrit.cardcrawl.unlock.UnlockTracker.achievementPref);
            assertSame(layout, com.megacrit.cardcrawl.helpers.FontHelper.layout);
            for (Map.Entry<Field, BitmapFont> entry : fonts.entrySet()) {
                assertSame(entry.getValue(), entry.getKey().get(null));
            }
            for (Map.Entry<Field, Texture> entry : textures.entrySet()) {
                assertSame(entry.getValue(), entry.getKey().get(null));
            }
        }
    }

    @Test
    public void productionListenerBlocksPrivateMatchTimerThenUnblocksAtZero() throws Exception {
        TestStateSnapshot statics = TestStateSnapshot.of(GameStateListener.class);
        FixtureSnapshot fixture = FixtureSnapshot.capture();
        try {
            installHeadlessGameDependencies(fixture);
            fixture.captureDungeon();
            configureDungeonFixture();
            EventRoom room = allocate(EventRoom.class);
            GremlinMatchGame event = allocate(GremlinMatchGame.class);
            room.event = event;
            room.phase = AbstractRoom.RoomPhase.COMPLETE;
            AbstractDungeon.currMapNode.room = room;
            ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "waitTimer", 0.5F);
            GameStateListener.resetStateVariables();
            assertFalse(GameStateListener.checkForDungeonStateChange());

            // Only the target event update may consume this timer; the
            // listener neither ticks it nor invents a replacement duration.
            ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "waitTimer", 0.0F);
            assertFalse(GameStateListener.checkForDungeonStateChange());
            assertTrue(GameStateListener.checkForDungeonStateChange());
        } finally {
            try {
                fixture.restoreGlobals();
            } finally {
                try {
                    fixture.restoreConfigFile();
                } finally {
                    statics.restore();
                }
            }
        }
    }

    @Test
    public void productionListenerBlocksSmokeUntilRoomCompletesAfterEscapeFlagClears() throws Exception {
        TestStateSnapshot statics = TestStateSnapshot.of(GameStateListener.class);
        FixtureSnapshot fixture = FixtureSnapshot.capture();
        try {
            installHeadlessGameDependencies(fixture);
            fixture.captureDungeon();
            configureDungeonFixture();
            AbstractDungeon.currMapNode.room.phase = AbstractRoom.RoomPhase.COMBAT;
            AbstractDungeon.currMapNode.room.monsters =
                    new MonsterGroup(new com.megacrit.cardcrawl.monsters.AbstractMonster[0]);
            AbstractDungeon.isScreenUp = false;
            AbstractDungeon.player.isEscaping = true;
            AbstractDungeon.player.endTurnQueued = false;
            AbstractDungeon.isFadingOut = false;
            AbstractDungeon.isFadingIn = false;
            GameStateListener.resetStateVariables();
            GameStateListener.signalTurnStart();
            assertFalse(GameStateListener.checkForDungeonStateChange());

            // endBattle clears the player flag before the room's subsequent
            // update completes the smoked reward transition. Even an apparent
            // screen boundary must not publish that intermediate combat state.
            AbstractDungeon.player.isEscaping = false;
            AbstractDungeon.currMapNode.room.smoked = true;
            AbstractDungeon.isScreenUp = true;
            assertFalse(GameStateListener.checkForDungeonStateChange());

            // Only the natural room update completes the transition. The
            // listener does not advance its timer or clear the smoked marker.
            AbstractDungeon.currMapNode.room.phase = AbstractRoom.RoomPhase.COMPLETE;
            assertFalse(GameStateListener.checkForDungeonStateChange());
            assertTrue(GameStateListener.checkForDungeonStateChange());
            assertTrue(AbstractDungeon.currMapNode.room.smoked);
        } finally {
            try {
                fixture.restoreGlobals();
            } finally {
                try {
                    fixture.restoreConfigFile();
                } finally {
                    statics.restore();
                }
            }
        }
    }

    @Test
    public void potionDiscardOnOpenCodexScreenSettlesDespiteQueuedEndTurn() throws Exception {
        TestStateSnapshot listenerStatics = TestStateSnapshot.of(GameStateListener.class);
        FixtureSnapshot fixture = FixtureSnapshot.capture();
        try {
            installHeadlessGameDependencies(fixture);
            fixture.captureDungeon();
            configureDungeonFixture();
            AbstractDungeon.currMapNode.room.phase = AbstractRoom.RoomPhase.COMBAT;
            AbstractDungeon.currMapNode.room.monsters =
                    new MonsterGroup(new com.megacrit.cardcrawl.monsters.AbstractMonster[0]);
            AbstractDungeon.screen = AbstractDungeon.CurrentScreen.CARD_REWARD;
            AbstractDungeon.isScreenUp = true;
            AbstractDungeon.isFadingIn = false;
            AbstractDungeon.isFadingOut = false;
            AbstractDungeon.player.endTurnQueued = true;
            AbstractDungeon.actionManager.currentAction = allocate(
                    com.megacrit.cardcrawl.actions.unique.CodexAction.class);
            GameStateListener.resetStateVariables();
            GameStateListener.signalTurnStart();
            assertTrue(GameStateListener.checkForDungeonStateChange());

            GameStateListener.beforeCommand("discard-during-codex", "potion");
            GameStateListener.registerStateChange(); // executePotionCommand's mutation signal
            assertTrue(GameStateListener.checkForDungeonStateChange());
            assertTrue(GameStateListener.isWaitingForCommand());
            assertTrue(AbstractDungeon.player.endTurnQueued);
            assertSame(AbstractDungeon.CurrentScreen.CARD_REWARD, AbstractDungeon.screen);
        } finally {
            try { fixture.restoreGlobals(); } finally {
                try { fixture.restoreConfigFile(); } finally { listenerStatics.restore(); }
            }
        }
    }

    @Test
    public void queuedEndTurnSettlesAtRetainInputNotQuiescence() throws Exception {
        FixtureSnapshot fixture = FixtureSnapshot.capture();
        try {
            installHeadlessGameDependencies(fixture);
            fixture.captureDungeon();
            configureDungeonFixture();
            AbstractDungeon.currMapNode.room.phase = AbstractRoom.RoomPhase.COMBAT;
            AbstractDungeon.currMapNode.room.monsters = new MonsterGroup(
                    new com.megacrit.cardcrawl.monsters.AbstractMonster[0]);
            AbstractDungeon.isScreenUp = false;
            GameStateListener.resetStateVariables();
            GameStateListener.signalTurnStart();
            long settledBefore = GameStateListener.getCommandSettlementSeq();
            GameStateListener.beforeCommand("end-retain", "end");
            AbstractDungeon.player.endTurnQueued = true;
            assertFalse(GameStateListener.checkForDungeonStateChange());

            // Target RetainCardPower.atEndOfTurn (Well-Laid Plans) queues this
            // owner; RetainCardsAction opens HAND_SELECT and queues WaitAction.
            com.megacrit.cardcrawl.actions.unique.RetainCardsAction owner = allocate(
                    com.megacrit.cardcrawl.actions.unique.RetainCardsAction.class);
            ReflectionHacks.setPrivate(owner, com.megacrit.cardcrawl.actions.AbstractGameAction.class, "duration", 0.4f);
            AbstractDungeon.actionManager.currentAction = owner;
            com.megacrit.cardcrawl.actions.utility.WaitAction following = allocate(
                    com.megacrit.cardcrawl.actions.utility.WaitAction.class);
            AbstractDungeon.actionManager.actions.add(following);
            AbstractDungeon.actionManager.phase = GameActionManager.Phase.EXECUTING_ACTIONS;
            AbstractDungeon.screen = AbstractDungeon.CurrentScreen.HAND_SELECT;
            AbstractDungeon.isScreenUp = true;
            assertTrue(GameStateListener.checkForDungeonStateChange());
            assertEquals(settledBefore + 1, GameStateListener.getCommandSettlementSeq());
            assertEquals("interaction_ready", GameStateListener.consumeBoundaryKind());
            assertEquals("end-retain", GameStateListener.getCommandResponseId());
            assertFalse(CommandExecutor.isEndCommandAvailable());

            GameStateListener.beforeCommand("discard-retain", "potion");
            GameStateListener.registerStateChange();
            AbstractGameEffect pending = allocate(ObtainPotionEffect.class);
            AbstractDungeon.effectsQueue.add(pending);
            assertFalse(GameStateListener.checkForDungeonStateChange());
            assertEquals(settledBefore + 1, GameStateListener.getCommandSettlementSeq());
            AbstractDungeon.effectsQueue.clear(); // test-only natural completion
            assertTrue(GameStateListener.checkForDungeonStateChange());
            assertEquals(settledBefore + 2, GameStateListener.getCommandSettlementSeq());
            assertEquals("interaction_ready", GameStateListener.consumeBoundaryKind());
            assertSame(owner, AbstractDungeon.actionManager.currentAction);
            assertSame(following, AbstractDungeon.actionManager.actions.get(0));
            assertTrue(AbstractDungeon.player.endTurnQueued);
            assertFalse(owner.isDone);
            assertEquals(0.4f, (Float) ReflectionHacks.getPrivate(owner,
                    com.megacrit.cardcrawl.actions.AbstractGameAction.class, "duration"), 0.0f);
        } finally {
            try { fixture.restoreGlobals(); } finally { fixture.restoreConfigFile(); }
        }
    }

    @Test
    public void unownedOrFinishedScreenCannotBypassQueuedEndTurn() throws Exception {
        FixtureSnapshot fixture = FixtureSnapshot.capture();
        try {
            installHeadlessGameDependencies(fixture);
            fixture.captureDungeon();
            configureDungeonFixture();
            AbstractDungeon.currMapNode.room.phase = AbstractRoom.RoomPhase.COMBAT;
            AbstractDungeon.currMapNode.room.monsters = new MonsterGroup(
                    new com.megacrit.cardcrawl.monsters.AbstractMonster[0]);
            AbstractDungeon.player.endTurnQueued = true;
            for (AbstractDungeon.CurrentScreen screen : Arrays.asList(
                    AbstractDungeon.CurrentScreen.CARD_REWARD,
                    AbstractDungeon.CurrentScreen.HAND_SELECT,
                    AbstractDungeon.CurrentScreen.MAP,
                    AbstractDungeon.CurrentScreen.GAME_DECK_VIEW)) {
                AbstractDungeon.screen = screen;
                AbstractDungeon.isScreenUp = true;
                AbstractDungeon.actionManager.currentAction = allocate(
                        com.megacrit.cardcrawl.actions.utility.WaitAction.class);
                GameStateListener.resetStateVariables();
                GameStateListener.signalTurnStart();
                assertTrue(GameStateListener.checkForDungeonStateChange());
                GameStateListener.beforeCommand("unowned-overlay", "potion");
                GameStateListener.registerStateChange();
                assertFalse(GameStateListener.checkForDungeonStateChange());
                assertTrue(GameStateListener.isTransactionPending());
            }
            AbstractDungeon.screen = AbstractDungeon.CurrentScreen.CARD_REWARD;
            AbstractDungeon.actionManager.currentAction = allocate(
                    com.megacrit.cardcrawl.actions.unique.CodexAction.class);
            AbstractDungeon.actionManager.currentAction.isDone = true;
            GameStateListener.resetStateVariables();
            GameStateListener.signalTurnStart();
            assertTrue(GameStateListener.checkForDungeonStateChange());
            GameStateListener.beforeCommand("finished-owner", "potion");
            GameStateListener.registerStateChange();
            assertFalse(GameStateListener.checkForDungeonStateChange());
        } finally {
            try { fixture.restoreGlobals(); } finally { fixture.restoreConfigFile(); }
        }
    }

    @Test
    public void rewardOfferIndicesStayStableAcrossPotionCapacityChanges() throws Exception {
        FixtureSnapshot fixture = FixtureSnapshot.capture();
        try {
            installHeadlessGameDependencies(fixture);
            fixture.captureDungeon();
            configureDungeonFixture();
            AbstractDungeon.screen = AbstractDungeon.CurrentScreen.COMBAT_REWARD;
            AbstractDungeon.isScreenUp = true;
            AbstractDungeon.combatRewardScreen = allocate(com.megacrit.cardcrawl.screens.CombatRewardScreen.class);
            com.megacrit.cardcrawl.rewards.RewardItem potion = allocate(com.megacrit.cardcrawl.rewards.RewardItem.class);
            potion.type = com.megacrit.cardcrawl.rewards.RewardItem.RewardType.POTION;
            com.megacrit.cardcrawl.rewards.RewardItem card = allocate(com.megacrit.cardcrawl.rewards.RewardItem.class);
            card.type = com.megacrit.cardcrawl.rewards.RewardItem.RewardType.CARD;
            AbstractDungeon.combatRewardScreen.rewards = new ArrayList<>(Arrays.asList(potion, card));
            AbstractDungeon.player.potions.add(fixturePotion());
            assertChoiceFixture("reward_full");
            assertChoiceRejected(0);
            assertFalse(potion.isDone);
            assertFalse(card.isDone);
            executeChoiceCommand(1);
            assertFalse(potion.isDone);
            assertTrue(card.isDone); // CHOOSE 1 still selects the card after the potion.
            card.isDone = false;
            AbstractDungeon.player.potions.set(0, allocate(PotionSlot.class));
            assertChoiceFixture("reward_open");
            AbstractDungeon.player.potions.set(0, fixturePotion());
            AbstractDungeon.combatRewardScreen.rewards.remove(card);
            assertChoiceFixture("reward_only_full");
        } finally {
            try { fixture.restoreGlobals(); } finally { fixture.restoreConfigFile(); }
        }
    }

    @Test
    public void shopOfferIndicesStayStableWithSozuAndFullBelt() throws Exception {
        FixtureSnapshot fixture = FixtureSnapshot.capture();
        TestStateSnapshot shopStatics = null;
        try {
            installHeadlessGameDependencies(fixture);
            fixture.captureDungeon();
            configureDungeonFixture();
            shopStatics = TestStateSnapshot.of(com.megacrit.cardcrawl.shop.ShopScreen.class);
            AbstractDungeon.screen = AbstractDungeon.CurrentScreen.SHOP;
            AbstractDungeon.isScreenUp = true;
            com.megacrit.cardcrawl.shop.ShopScreen shop = allocate(com.megacrit.cardcrawl.shop.ShopScreen.class);
            AbstractDungeon.shopScreen = shop;
            shop.purgeAvailable = true;
            com.megacrit.cardcrawl.shop.ShopScreen.actualPurgeCost = 25;
            shop.coloredCards = new ArrayList<>();
            shop.colorlessCards = new ArrayList<>();
            ReflectionHacks.setPrivate(shop, com.megacrit.cardcrawl.shop.ShopScreen.class, "relics", new ArrayList<>());
            com.megacrit.cardcrawl.shop.StorePotion offer = allocate(com.megacrit.cardcrawl.shop.StorePotion.class);
            offer.potion = fixturePotion();
            offer.price = 7;
            ReflectionHacks.setPrivate(shop, com.megacrit.cardcrawl.shop.ShopScreen.class, "potions",
                    new ArrayList<>(Arrays.asList(offer)));
            AbstractDungeon.player.potions.add(allocate(PotionSlot.class));
            com.megacrit.cardcrawl.relics.Sozu sozu = allocate(com.megacrit.cardcrawl.relics.Sozu.class);
            Field id = com.megacrit.cardcrawl.relics.AbstractRelic.class.getDeclaredField("relicId");
            id.setAccessible(true);
            id.set(sozu, com.megacrit.cardcrawl.relics.Sozu.ID);
            AbstractDungeon.player.relics.add(sozu);
            assertChoiceFixture("shop_sozu");
            assertChoiceRejected(1);
            AbstractDungeon.player.relics.clear();
            AbstractDungeon.player.potions.set(0, fixturePotion());
            assertChoiceFixture("shop_full");
            assertChoiceRejected(1);
            AbstractDungeon.player.potions.set(0, allocate(PotionSlot.class));
            assertChoiceFixture("shop_open");
            assertFalse(offer.isPurchased);
        } finally {
            try { fixture.restoreGlobals(); } finally {
                try { fixture.restoreConfigFile(); } finally { if (shopStatics != null) shopStatics.restore(); }
            }
        }
    }

    private static AbstractPotion fixturePotion() throws Exception {
        com.megacrit.cardcrawl.potions.FirePotion potion = allocate(com.megacrit.cardcrawl.potions.FirePotion.class);
        potion.name = "Fire Potion";
        return potion;
    }

    private static void assertChoiceFixture(String name) throws Exception {
        JsonObject fixtures;
        try (java.io.Reader reader = new java.io.InputStreamReader(
                PendingGameplayEffectTest.class.getResourceAsStream("/choice-contract.json"), "UTF-8")) {
            fixtures = new Gson().fromJson(reader, JsonObject.class);
        }
        JsonObject expected = fixtures.getAsJsonObject("cases").getAsJsonObject(name);
        HashMap<String, Object> state = new HashMap<>();
        state.put("screen_type", ChoiceScreenUtils.getCurrentChoiceType().name());
        GameStateConverter.addChoiceState(state);
        assertEquals(expected.getAsJsonObject("game_state"), new Gson().toJsonTree(state));
        boolean chooses = expected.getAsJsonArray("available_commands").toString().contains("\"choose\"");
        assertEquals(chooses, CommandExecutor.isChooseCommandAvailable());
    }

    private static void executeChoiceCommand(int index) throws Exception {
        java.lang.reflect.Method method = CommandExecutor.class.getDeclaredMethod("executeChooseCommand", String[].class);
        method.setAccessible(true);
        method.invoke(null, (Object) new String[]{"choose", Integer.toString(index)});
    }

    private static void assertChoiceRejected(int index) throws Exception {
        try {
            executeChoiceCommand(index);
            org.junit.Assert.fail("nonselectable choice was executed");
        } catch (java.lang.reflect.InvocationTargetException exception) {
            assertTrue(exception.getCause() instanceof InvalidCommandException);
        }
    }

    @Test
    public void smithCancellationWaitsForNaturalCompletionButAllowsOpenGrid() throws Exception {
        assertSelectionEffectLifecycle(CampfireSmithEffect.class, "smith");
    }

    @Test
    public void tokeCancellationWaitsForNaturalCompletionButAllowsOpenGrid() throws Exception {
        assertSelectionEffectLifecycle(CampfireTokeEffect.class, "toke");
    }

    private void assertSelectionEffectLifecycle(Class<? extends AbstractGameEffect> effectClass, String option) throws Exception {
        FixtureSnapshot fixture = FixtureSnapshot.capture();
        try {
            installHeadlessGameDependencies(fixture);
            fixture.captureDungeon();
            configureDungeonFixture();
            for (ArrayList<AbstractGameEffect> queue : Arrays.asList(AbstractDungeon.effectList,
                    AbstractDungeon.effectsQueue, AbstractDungeon.topLevelEffects, AbstractDungeon.topLevelEffectsQueue)) {
                AbstractGameEffect effect = allocate(effectClass);
                effect.duration = 0.5f;
                effect.isDone = false;
                queue.add(effect);
                AbstractDungeon.screen = AbstractDungeon.CurrentScreen.GRID;
                AbstractDungeon.isScreenUp = true;
                GameStateListener.resetStateVariables();
                assertFalse(GameStateListener.checkForDungeonStateChange());
                assertTrue(GameStateListener.checkForDungeonStateChange());
                // The effect's timer is paused while the user is choosing.
                assertEquals(0.5f, effect.duration, 0.0f);
                assertFalse(effect.isDone);
                AbstractDungeon.screen = AbstractDungeon.CurrentScreen.NONE;
                AbstractDungeon.isScreenUp = false;
                GameStateListener.beforeCommand("cancel-" + option, "cancel");
                assertFalse(GameStateListener.checkForDungeonStateChange());
                assertFalse(GameStateListener.checkForDungeonStateChange());
                assertEquals(0.5f, effect.duration, 0.0f);
                assertFalse(effect.isDone);
                assertEquals(1, GameStateListener.getEffectQueueSize()
                        + GameStateListener.getQueuedEffectQueueSize()
                        + GameStateListener.getTopLevelEffectQueueSize()
                        + GameStateListener.getQueuedTopLevelEffectQueueSize());
                // Test-only completion: production must wait, never update or repair the effect.
                effect.isDone = true;
                assertFalse(GameStateListener.checkForDungeonStateChange());
                assertTrue(GameStateListener.checkForDungeonStateChange());
                queue.remove(effect);
            }
        } finally {
            try { fixture.restoreGlobals(); } finally { fixture.restoreConfigFile(); }
        }
    }

    private static void configureDungeonFixture() throws Exception {
        AbstractDungeon.player = minimalPlayer();
        AbstractRoom room = allocate(RestRoom.class);
        room.phase = AbstractRoom.RoomPhase.COMPLETE;
        MapRoomNode node = allocate(MapRoomNode.class);
        node.room = room;
        AbstractDungeon.currMapNode = node;
        AbstractDungeon.actionManager = new GameActionManager();
        AbstractDungeon.actionManager.phase = GameActionManager.Phase.WAITING_ON_USER;
        AbstractDungeon.effectList = new ArrayList<>();
        AbstractDungeon.effectsQueue = new ArrayList<>();
        AbstractDungeon.topLevelEffects = new ArrayList<>();
        AbstractDungeon.topLevelEffectsQueue = new ArrayList<>();
        AbstractDungeon.map = new ArrayList<>();
        AbstractDungeon.screen = AbstractDungeon.CurrentScreen.NONE;
        AbstractDungeon.isScreenUp = true;
        AbstractDungeon.gridSelectScreen = allocate(
                com.megacrit.cardcrawl.screens.select.GridCardSelectScreen.class);
        CardCrawlGame.mode = CardCrawlGame.GameMode.GAMEPLAY;
        CardCrawlGame.dungeon = allocate(TheCity.class);
    }

    private static AbstractPlayer minimalPlayer() throws InstantiationException {
        AbstractPlayer player = allocate(Ironclad.class);
        player.chosenClass = AbstractPlayer.PlayerClass.IRONCLAD;
        player.currentHealth = 70;
        player.maxHealth = 70;
        player.gold = 99;
        player.masterDeck = new CardGroup(CardGroup.CardGroupType.MASTER_DECK);
        player.drawPile = new CardGroup(CardGroup.CardGroupType.DRAW_PILE);
        player.hand = new CardGroup(CardGroup.CardGroupType.HAND);
        player.discardPile = new CardGroup(CardGroup.CardGroupType.DISCARD_PILE);
        player.exhaustPile = new CardGroup(CardGroup.CardGroupType.EXHAUST_PILE);
        player.limbo = new CardGroup(CardGroup.CardGroupType.UNSPECIFIED);
        player.relics = new ArrayList<>();
        player.blights = new ArrayList<>();
        player.potions = new ArrayList<>();
        return player;
    }

    private static JsonObject stateJson() {
        return new Gson().fromJson(GameStateConverter.getCommunicationState(), JsonObject.class);
    }

    private static void installHeadlessGameDependencies(FixtureSnapshot snapshot) throws Exception {
        TestLocalizedStrings languagePack = allocate(TestLocalizedStrings.class);
        languagePack.uiStrings = emptyUiStrings();
        CardCrawlGame.languagePack = languagePack;
        Gdx.files = proxy(Files.class);
        Gdx.graphics = proxy(Graphics.class);
        com.megacrit.cardcrawl.core.Settings.gamePref = new Prefs();
        com.megacrit.cardcrawl.core.Settings.soundPref = new Prefs();
        com.megacrit.cardcrawl.unlock.UnlockTracker.achievementPref = new Prefs();
        Gdx.input = proxy(Input.class);
        installImageMasterTextures(snapshot.textures);
        BitmapFont.BitmapFontData data = new BitmapFont.BitmapFontData();
        data.imagePaths = new String[0];
        data.markupEnabled = false;
        BitmapFont font = new BitmapFont(
                data,
                new Array<TextureRegion>(new TextureRegion[]{new TextureRegion()}),
                false);
        for (Field field : com.megacrit.cardcrawl.helpers.FontHelper.class.getFields()) {
            if (field.getType() == BitmapFont.class) {
                field.set(null, font);
            }
        }
        com.megacrit.cardcrawl.helpers.FontHelper.layout = new GlyphLayout();
    }

    private static void installImageMasterTextures(Map<Field, Texture> originalTextures) throws Exception {
        Texture texture = allocate(Texture.class);
        Field dataField = Texture.class.getDeclaredField("data");
        UNSAFE.putObject(texture, UNSAFE.objectFieldOffset(dataField), proxy(TextureData.class));
        for (Field field : originalTextures.keySet()) {
            field.set(null, texture);
        }
    }

    @SuppressWarnings("unchecked")
    private static <T> T proxy(Class<T> type) {
        return (T) Proxy.newProxyInstance(
                PendingGameplayEffectTest.class.getClassLoader(),
                new Class<?>[]{type},
                (proxy, method, args) -> {
                    if (method.getReturnType() == FileHandle.class) {
                        return new FileHandle("/tmp/communication-mod-test-missing");
                    }
                    if (method.getReturnType() == Graphics.DisplayMode.class) {
                        return displayMode();
                    }
                    if (method.getReturnType() == Graphics.DisplayMode[].class) {
                        return new Graphics.DisplayMode[]{displayMode()};
                    }
                    if (method.getReturnType() == boolean.class) {
                        return false;
                    }
                    if (method.getReturnType() == int.class) {
                        return 0;
                    }
                    if (method.getReturnType() == long.class) {
                        return 0L;
                    }
                    if (method.getReturnType() == float.class) {
                        return 0.0f;
                    }
                    if (method.getReturnType() == double.class) {
                        return 0.0d;
                    }
                    return null;
                });
    }

    private static Graphics.DisplayMode displayMode() {
        try {
            Graphics.DisplayMode mode = allocate(Graphics.DisplayMode.class);
            putInt(mode, "width", 800);
            putInt(mode, "height", 600);
            putInt(mode, "refreshRate", 60);
            putInt(mode, "bitsPerPixel", 32);
            return mode;
        } catch (Exception exception) {
            throw new AssertionError(exception);
        }
    }

    private static void putInt(Object target, String fieldName, int value)
            throws ReflectiveOperationException {
        Field field = target.getClass().getField(fieldName);
        UNSAFE.putInt(target, UNSAFE.objectFieldOffset(field), value);
    }

    private static final class TestLocalizedStrings extends LocalizedStrings {
        private UIStrings uiStrings;

        @Override
        public EventStrings getEventString(String key) {
            EventStrings strings = new EventStrings();
            strings.NAME = "test";
            strings.DESCRIPTIONS = new String[]{"test", "test", "test"};
            strings.OPTIONS = new String[]{"test", "test", "test", "test"};
            return strings;
        }

        @Override
        public UIStrings getUIString(String key) {
            return uiStrings;
        }

        @Override
        public CharacterStrings getCharacterString(String key) {
            CharacterStrings strings = new CharacterStrings();
            strings.NAMES = new String[100];
            strings.TEXT = new String[100];
            strings.OPTIONS = new String[100];
            strings.UNIQUE_REWARDS = new String[100];
            Arrays.fill(strings.NAMES, "");
            Arrays.fill(strings.TEXT, "");
            Arrays.fill(strings.OPTIONS, "");
            Arrays.fill(strings.UNIQUE_REWARDS, "");
            return strings;
        }

        @Override
        public com.megacrit.cardcrawl.localization.PotionStrings getPotionString(String key) {
            com.megacrit.cardcrawl.localization.PotionStrings strings = new com.megacrit.cardcrawl.localization.PotionStrings();
            strings.NAME = key;
            strings.DESCRIPTIONS = new String[]{"fixture"};
            return strings;
        }

        @Override
        public com.megacrit.cardcrawl.localization.RelicStrings getRelicStrings(String key) {
            com.megacrit.cardcrawl.localization.RelicStrings strings = new com.megacrit.cardcrawl.localization.RelicStrings();
            strings.NAME = key;
            strings.FLAVOR = "fixture";
            strings.DESCRIPTIONS = new String[]{"fixture"};
            return strings;
        }

        @Override
        public TutorialStrings getTutorialString(String key) {
            TutorialStrings strings = new TutorialStrings();
            strings.TEXT = new String[100];
            strings.LABEL = new String[100];
            Arrays.fill(strings.TEXT, "");
            Arrays.fill(strings.LABEL, "");
            return strings;
        }
    }

    private static UIStrings emptyUiStrings() {
        UIStrings strings = new UIStrings();
        strings.TEXT = new String[100];
        Arrays.fill(strings.TEXT, "");
        return strings;
    }
}
