package communicationmod;

import basemod.ReflectionHacks;
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
import com.megacrit.cardcrawl.cards.AbstractCard;
import com.megacrit.cardcrawl.cards.CardGroup;
import com.megacrit.cardcrawl.core.CardCrawlGame;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.events.shrines.GremlinMatchGame;
import com.megacrit.cardcrawl.localization.CharacterStrings;
import com.megacrit.cardcrawl.localization.EventStrings;
import com.megacrit.cardcrawl.localization.LocalizedStrings;
import com.megacrit.cardcrawl.localization.UIStrings;
import com.megacrit.cardcrawl.localization.TutorialStrings;
import com.megacrit.cardcrawl.helpers.Prefs;
import com.megacrit.cardcrawl.helpers.Hitbox;
import com.megacrit.cardcrawl.helpers.input.InputHelper;
import com.megacrit.cardcrawl.characters.AbstractPlayer;
import com.megacrit.cardcrawl.monsters.AbstractMonster;
import communicationmod.patches.GremlinMatchGamePatch;
import org.junit.Test;
import sun.misc.Unsafe;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.UUID;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.nio.file.Paths;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertSame;
import static org.junit.Assert.assertTrue;

public class GremlinMatchGamePatchTest {
    private static Unsafe unsafe() throws ReflectiveOperationException {
        Field field = Unsafe.class.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        return (Unsafe) field.get(null);
    }

    private static final class TestLocalizedStrings extends LocalizedStrings {
        private UIStrings uiStrings;

        @Override
        public UIStrings getUIString(String key) {
            if (uiStrings == null) {
                uiStrings = new UIStrings();
                uiStrings.TEXT = new String[100];
            }
            return uiStrings;
        }

        @Override
        public CharacterStrings getCharacterString(String key) {
            CharacterStrings strings = new CharacterStrings();
            strings.NAMES = new String[100];
            strings.TEXT = new String[100];
            strings.OPTIONS = new String[100];
            strings.UNIQUE_REWARDS = new String[100];
            java.util.Arrays.fill(strings.NAMES, "test");
            java.util.Arrays.fill(strings.TEXT, "test");
            java.util.Arrays.fill(strings.OPTIONS, "test");
            java.util.Arrays.fill(strings.UNIQUE_REWARDS, "test");
            return strings;
        }

        @Override
        public TutorialStrings getTutorialString(String key) {
            TutorialStrings strings = new TutorialStrings();
            strings.TEXT = new String[100];
            strings.LABEL = new String[100];
            return strings;
        }

        @Override
        public EventStrings getEventString(String key) {
            EventStrings strings = new EventStrings();
            strings.NAME = "Match and Keep!";
            strings.DESCRIPTIONS = new String[]{"description", "complete"};
            strings.OPTIONS = new String[]{"play", "leave", "rule", "attempts"};
            return strings;
        }
    }

    private static final class TestCard extends AbstractCard {
        private TestCard() {
            super("test", "test", null, 0, "", CardType.SKILL, CardColor.COLORLESS,
                    CardRarity.SPECIAL, CardTarget.NONE);
        }

        @Override
        public void upgrade() {
        }

        @Override
        public void use(AbstractPlayer player, AbstractMonster monster) {
        }

        @Override
        public AbstractCard makeCopy() {
            return this;
        }
    }

    @Test
    public void chooseQueuesNaturalTargetUpdateWithoutMutatingCardOrEvent() throws Exception {
        TestStateSnapshot statics = TestStateSnapshot.of(
                CardCrawlGame.class, InputHelper.class, GameStateListener.class,
                GremlinMatchGamePatch.class, GremlinMatchGamePatch.HoverCardPatch.class);
        LocalizedStrings oldLanguagePack = CardCrawlGame.languagePack;
        CardCrawlGame.languagePack = testLanguage();
        CardGroup oldCards = GremlinMatchGamePatch.cards;
        HashMap<UUID, Integer> oldPositions = GremlinMatchGamePatch.cardPositions;
        AbstractCard oldHover = GremlinMatchGamePatch.HoverCardPatch.hoverCard;
        boolean oldDoHover = GremlinMatchGamePatch.HoverCardPatch.doHover;
        boolean oldClicked = InputHelper.justClickedLeft;
        try {
            TestCard card = (TestCard) unsafe().allocateInstance(TestCard.class);
            card.uuid = UUID.randomUUID();
            card.hb = new Hitbox(1.0F, 1.0F);
            card.isFlipped = true;
            CardGroup cards = new CardGroup(CardGroup.CardGroupType.UNSPECIFIED);
            cards.group = new ArrayList<>();
            cards.group.add(card);
            GremlinMatchGamePatch.cards = cards;
            GremlinMatchGamePatch.cardPositions = new HashMap<>();
            GremlinMatchGamePatch.cardPositions.put(card.uuid, 0);
            GremlinMatchGame event = (GremlinMatchGame) unsafe().allocateInstance(GremlinMatchGame.class);
            GremlinMatchGamePatch.HoverCardPatch.doHover = false;
            GremlinMatchGamePatch.HoverCardPatch.hoverCard = null;
            InputHelper.justClickedLeft = false;

            GremlinMatchGamePatch.chooseFaceDownCard(event, 0);

            assertTrue(card.isFlipped);
            assertTrue(GremlinMatchGamePatch.HoverCardPatch.doHover);
            assertSame(card, GremlinMatchGamePatch.HoverCardPatch.hoverCard);
            assertTrue(InputHelper.justClickedLeft);

            ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "waitTimer", 0.5F);
            assertTrue(GremlinMatchGamePatch.hasPendingMatchWait(event));
            GremlinMatchGamePatch.HoverCardPatch.Insert(event, card);
            assertTrue(GremlinMatchGamePatch.HoverCardPatch.doHover);
            assertFalse(card.hb.hovered);

            ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "waitTimer", 0.0F);
            assertFalse(GremlinMatchGamePatch.hasPendingMatchWait(event));
            ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "waitTimer", -0.1F);
            assertFalse(GremlinMatchGamePatch.hasPendingMatchWait(event));
            ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "waitTimer", 0.0F);
            // This is the production insert point immediately after the
            // target Hitbox.update() call. It consumes the queued selection;
            // the target update then owns all card/event mutations.
            GremlinMatchGamePatch.HoverCardPatch.Insert(event, card);
            assertFalse(GremlinMatchGamePatch.HoverCardPatch.doHover);
            assertTrue(card.hb.hovered);
            assertTrue(card.hb.clicked);
            assertTrue(card.isFlipped);
        } finally {
            GremlinMatchGamePatch.cards = oldCards;
            GremlinMatchGamePatch.cardPositions = oldPositions;
            GremlinMatchGamePatch.HoverCardPatch.hoverCard = oldHover;
            GremlinMatchGamePatch.HoverCardPatch.doHover = oldDoHover;
            InputHelper.justClickedLeft = oldClicked;
            CardCrawlGame.languagePack = oldLanguagePack;
            statics.restore();
        }
    }

    private enum HeadlessScenario {
        MISMATCH,
        MATCH,
        FINAL_DIALOG
    }

    @Test
    public void targetUpdateFlipsBothCardsBackAfterMismatch() throws Exception {
        runHeadlessScenario(HeadlessScenario.MISMATCH);
    }

    @Test
    public void targetUpdatePublishesObtainEffectForMatchingPair() throws Exception {
        runHeadlessScenario(HeadlessScenario.MATCH);
    }

    @Test
    public void targetUpdateReachesFinalDialogStateAfterLastAttempt() throws Exception {
        runHeadlessScenario(HeadlessScenario.FINAL_DIALOG);
    }

    /**
     * The order here is intentional: these are separate fixture lifetimes in one
     * JVM, including a repeated final-dialog run. The sentinels make restoring a
     * newly-created headless value to another newly-created value observable.
     */
    @Test
    public void headlessFixtureRestoresGlobalsAcrossReorderedRepeatedScenarios() throws Exception {
        ConfigSnapshot ownerConfig = ConfigSnapshot.capture();
        TestStateSnapshot ownerStatics = captureFixtureStatics();
        try {
            installSentinelGlobals();
            TestStateSnapshot sentinel = captureFixtureStatics();
            runHeadlessScenario(HeadlessScenario.FINAL_DIALOG);
            sentinel.assertRestored();
            runHeadlessScenario(HeadlessScenario.MISMATCH);
            sentinel.assertRestored();
            runHeadlessScenario(HeadlessScenario.MATCH);
            sentinel.assertRestored();
            runHeadlessScenario(HeadlessScenario.FINAL_DIALOG);
            sentinel.assertRestored();
        } finally {
            try {
                ownerStatics.restore();
            } finally {
                ownerConfig.restore();
            }
        }
        ownerConfig.assertRestored();
    }

    private static void runHeadlessScenario(HeadlessScenario scenario) throws Exception {
        // Capture the file before touching Settings: its first class initialization
        // may bootstrap vanilla preferences and create info.displayconfig.
        ConfigSnapshot config = ConfigSnapshot.capture();
        // Reflection can initialize an otherwise-uninitialized vanilla class. That
        // one-time bootstrap is not reversible; all synthetic fixture assignment is
        // deliberately after this baseline capture and is restored below.
        TestStateSnapshot statics = captureFixtureStatics();
        try {
            prepareHeadlessGdx();
            CardCrawlGame.languagePack = testLanguage();
            switch (scenario) {
                case MISMATCH:
                    assertMismatchScenario();
                    break;
                case MATCH:
                    assertMatchScenario();
                    break;
                case FINAL_DIALOG:
                    assertFinalDialogScenario();
                    break;
                default:
                    throw new AssertionError(scenario);
            }
        } finally {
            try {
                statics.restore();
                statics.assertRestored();
            } finally {
                config.restore();
            }
            config.assertRestored();
        }
    }

    private static TestStateSnapshot captureFixtureStatics() throws Exception {
        // AbstractDungeon's static initializer dereferences the language pack. If
        // it is the first use in this JVM, bootstrap only that class with a
        // temporary language pack, then restore the pre-bootstrap statics before
        // taking the actual fixture baseline. Class initialization itself cannot
        // be undone; the temporary assignment must not become the baseline.
        ConfigSnapshot bootstrapConfig = ConfigSnapshot.capture();
        TestStateSnapshot preBootstrap = null;
        try {
            preBootstrap = TestStateSnapshot.of(
                    Gdx.class, InputHelper.class, GameStateListener.class, CardCrawlGame.class,
                    GremlinMatchGamePatch.class, GremlinMatchGamePatch.HoverCardPatch.class,
                    com.megacrit.cardcrawl.core.Settings.class,
                    com.megacrit.cardcrawl.unlock.UnlockTracker.class,
                    com.megacrit.cardcrawl.helpers.FontHelper.class,
                    com.megacrit.cardcrawl.helpers.ImageMaster.class,
                    com.megacrit.cardcrawl.events.GenericEventDialog.class);
            // Vanilla TopPanel initialization also requires the headless Gdx
            // seams. These assignments are bootstrap-only and are rolled back
            // together with the temporary language pack below.
            prepareHeadlessGdx();
            CardCrawlGame.languagePack = testLanguage();
            Class.forName(AbstractDungeon.class.getName(), true,
                    AbstractDungeon.class.getClassLoader());
        } finally {
            try {
                if (preBootstrap != null) {
                    preBootstrap.restore();
                }
            } finally {
                bootstrapConfig.restore();
            }
        }
        return TestStateSnapshot.of(
                Gdx.class, InputHelper.class, AbstractDungeon.class, GameStateListener.class,
                CardCrawlGame.class, GremlinMatchGamePatch.class,
                GremlinMatchGamePatch.HoverCardPatch.class,
                com.megacrit.cardcrawl.core.Settings.class,
                com.megacrit.cardcrawl.unlock.UnlockTracker.class,
                com.megacrit.cardcrawl.helpers.FontHelper.class,
                com.megacrit.cardcrawl.helpers.ImageMaster.class,
                com.megacrit.cardcrawl.events.GenericEventDialog.class);
    }

    private static void assertMismatchScenario() throws Exception {
        AbstractCard first = card("first");
        AbstractCard second = card("second");
        GremlinMatchGame event = matchEvent(first, second, 1.0F, 5);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "chosenCard", first);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "hoveredCard", second);
        invokeTargetLogic(event);

        assertTrue(first.isFlipped);
        assertTrue(second.isFlipped);
        assertEquals(4, (int) ReflectionHacks.getPrivate(
                event, GremlinMatchGame.class, "attemptCount"));
        assertFalse((Boolean) ReflectionHacks.getPrivate(
                event, GremlinMatchGame.class, "cardFlipped"));
        assertFalse(GremlinMatchGamePatch.hasPendingMatchWait(event));
    }

    private static void assertMatchScenario() throws Exception {
        AbstractCard first = card("same");
        AbstractCard second = card("same");
        GremlinMatchGame event = matchEvent(first, second, 1.0F, 5);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "chosenCard", first);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "hoveredCard", second);
        invokeTargetLogic(event);

        assertEquals(0, ((CardGroup) ReflectionHacks.getPrivate(
                event, GremlinMatchGame.class, "cards")).group.size());
        assertEquals(1, AbstractDungeon.effectList.size());
        assertTrue(AbstractDungeon.effectList.get(0)
                instanceof com.megacrit.cardcrawl.vfx.cardManip.ShowCardAndObtainEffect);
        assertFalse(GremlinMatchGamePatch.hasPendingMatchWait(event));
    }

    private static void assertFinalDialogScenario() throws Exception {
        AbstractCard first = card("first");
        AbstractCard second = card("second");
        GremlinMatchGame event = matchEvent(first, second, 1.0F, 1);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "chosenCard", first);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "hoveredCard", second);
        invokeTargetLogic(event);
        assertTrue((Boolean) ReflectionHacks.getPrivate(event, GremlinMatchGame.class, "gameDone"));

        // This explicitly covers the CLEAN_UP phase; it does not claim the full
        // natural lifecycle because the test supplies that phase directly.
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "screen",
                Enum.valueOf((Class) Class.forName(
                        "com.megacrit.cardcrawl.events.shrines.GremlinMatchGame$CUR_SCREEN"), "CLEAN_UP"));
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "waitTimer", 1.0F);
        setInherited(event, "imageEventText", new com.megacrit.cardcrawl.events.GenericEventDialog());
        ((CardGroup) ReflectionHacks.getPrivate(event, GremlinMatchGame.class, "cards")).group.clear();
        event.update();
        assertEquals("COMPLETE", ReflectionHacks.getPrivate(
                event, GremlinMatchGame.class, "screen").toString());
        assertEquals(0.0F, (Float) ReflectionHacks.getPrivate(
                event, GremlinMatchGame.class, "waitTimer"), 0.0F);
    }

    private static void installSentinelGlobals() throws Exception {
        Gdx.files = (Files) Proxy.newProxyInstance(
                GremlinMatchGamePatchTest.class.getClassLoader(),
                new Class<?>[]{Files.class},
                (proxy, method, args) -> defaultValue(method.getReturnType()));
        Gdx.graphics = graphicsWithDelta(17.0F);
        Gdx.input = (Input) Proxy.newProxyInstance(
                GremlinMatchGamePatchTest.class.getClassLoader(),
                new Class<?>[]{Input.class},
                (proxy, method, args) -> defaultValue(method.getReturnType()));
        installHeadlessFontAndTextures();
        setStatic(com.megacrit.cardcrawl.events.GenericEventDialog.class,
                "words", new ArrayList<>());
        setStatic(com.megacrit.cardcrawl.events.GenericEventDialog.class, "show", true);
        setStatic(com.megacrit.cardcrawl.events.GenericEventDialog.class, "selectedOption", 37);
        setStatic(com.megacrit.cardcrawl.events.GenericEventDialog.class, "waitForInput", true);
        GameStateListener.registerStateChange();
        GameStateListener.setTimeout(37);
        GameStateListener.blockStateUpdate();
        GameStateListener.signalTurnStart();
    }

    private static void setStatic(Class<?> type, String name, Object value) throws Exception {
        Field field = type.getDeclaredField(name);
        field.setAccessible(true);
        field.set(null, value);
    }

    private static final class ConfigSnapshot {
        private final Path path;
        private final boolean exists;
        private final byte[] bytes;

        private ConfigSnapshot(Path path, boolean exists, byte[] bytes) {
            this.path = path;
            this.exists = exists;
            this.bytes = bytes;
        }

        private static ConfigSnapshot capture() throws Exception {
            Path path = Paths.get(System.getProperty("user.dir"), "info.displayconfig");
            boolean exists = java.nio.file.Files.exists(path);
            return new ConfigSnapshot(path, exists, exists ? java.nio.file.Files.readAllBytes(path) : null);
        }

        private void restore() throws Exception {
            if (exists) {
                java.nio.file.Files.write(path, bytes);
            } else {
                // Only a file created by this fixture can be removed here: an
                // absent owner file was never deleted or replaced.
                java.nio.file.Files.deleteIfExists(path);
            }
        }

        private void assertRestored() throws Exception {
            assertEquals(exists, java.nio.file.Files.exists(path));
            if (exists) {
                assertArrayEquals(bytes, java.nio.file.Files.readAllBytes(path));
            }
        }
    }

    private static TestLocalizedStrings testLanguage() throws Exception {
        TestLocalizedStrings strings = (TestLocalizedStrings) unsafe().allocateInstance(TestLocalizedStrings.class);
        strings.uiStrings = new UIStrings();
        strings.uiStrings.TEXT = new String[100];
        java.util.Arrays.fill(strings.uiStrings.TEXT, "test");
        return strings;
    }

    private static AbstractCard card(String id) throws Exception {
        TestCard card = (TestCard) unsafe().allocateInstance(TestCard.class);
        card.cardID = id;
        card.uuid = UUID.randomUUID();
        card.hb = new Hitbox(1.0F, 1.0F);
        card.rarity = AbstractCard.CardRarity.SPECIAL;
        card.color = AbstractCard.CardColor.COLORLESS;
        card.type = AbstractCard.CardType.SKILL;
        card.isFlipped = false;
        return card;
    }

    private static GremlinMatchGame matchEvent(AbstractCard first, AbstractCard second,
                                                float waitTimer, int attempts) throws Exception {
        GremlinMatchGame event = (GremlinMatchGame) unsafe().allocateInstance(GremlinMatchGame.class);
        CardGroup cards = new CardGroup(CardGroup.CardGroupType.UNSPECIFIED);
        cards.group = new ArrayList<>();
        cards.group.add(first);
        cards.group.add(second);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "cards", cards);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "waitTimer", waitTimer);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "attemptCount", attempts);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "gameDone", false);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "cardFlipped", false);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "matchedCards", new ArrayList<String>());
        AbstractDungeon.effectList = new ArrayList<>();
        Gdx.graphics = graphicsWithDelta(2.0F);
        return event;
    }

    private static void setInherited(Object target, String name, Object value) throws Exception {
        Class<?> type = target.getClass();
        while (type != null) {
            try {
                Field field = type.getDeclaredField(name);
                field.setAccessible(true);
                field.set(target, value);
                return;
            } catch (NoSuchFieldException ignored) {
                type = type.getSuperclass();
            }
        }
        throw new NoSuchFieldException(name);
    }

    private static void invokeTargetLogic(GremlinMatchGame event) throws Exception {
        Method method = GremlinMatchGame.class.getDeclaredMethod("updateMatchGameLogic");
        method.setAccessible(true);
        method.invoke(event);
    }

    private static void prepareHeadlessGdx() throws Exception {
        Gdx.files = (Files) Proxy.newProxyInstance(
                GremlinMatchGamePatchTest.class.getClassLoader(),
                new Class<?>[]{Files.class},
                (proxy, method, args) -> method.getReturnType() == FileHandle.class
                        ? new FileHandle("/tmp/communication-mod-test-missing")
                        : defaultValue(method.getReturnType()));
        Gdx.graphics = graphicsWithDelta(2.0F);
        com.megacrit.cardcrawl.core.Settings.gamePref = new Prefs();
        com.megacrit.cardcrawl.core.Settings.soundPref = new Prefs();
        com.megacrit.cardcrawl.unlock.UnlockTracker.achievementPref = new Prefs();
        installHeadlessFontAndTextures();
        installHeadlessVfxAtlas();
    }

    private static void installHeadlessVfxAtlas() throws Exception {
        com.badlogic.gdx.graphics.g2d.TextureAtlas atlas =
                new com.badlogic.gdx.graphics.g2d.TextureAtlas();
        Texture texture = null;
        for (Field field : com.megacrit.cardcrawl.helpers.ImageMaster.class.getDeclaredFields()) {
            field.setAccessible(true);
            if (field.getType() == Texture.class) {
                texture = (Texture) field.get(null);
                break;
            }
        }
        for (String name : new String[]{"eventBgParticle1", "eventBgParticle2"}) {
            com.badlogic.gdx.graphics.g2d.TextureAtlas.AtlasRegion region =
                    atlas.addRegion(name, new TextureRegion(texture));
            unsafe().putInt(region, unsafe().objectFieldOffset(
                    region.getClass().getField("packedWidth")), 64);
            unsafe().putInt(region, unsafe().objectFieldOffset(
                    region.getClass().getField("packedHeight")), 64);
        }
        setStatic(com.megacrit.cardcrawl.helpers.ImageMaster.class, "vfxAtlas", atlas);
    }

    private static void installHeadlessFontAndTextures() throws Exception {
        BitmapFont.BitmapFontData data = new BitmapFont.BitmapFontData();
        data.imagePaths = new String[0];
        data.markupEnabled = false;
        BitmapFont font = new BitmapFont(
                data, new Array<TextureRegion>(new TextureRegion[]{new TextureRegion()}), false);
        for (Field field : com.megacrit.cardcrawl.helpers.FontHelper.class.getFields()) {
            if (field.getType() == BitmapFont.class) {
                field.set(null, font);
            }
        }
        com.megacrit.cardcrawl.helpers.FontHelper.layout = new GlyphLayout();
        Texture texture = (Texture) unsafe().allocateInstance(Texture.class);
        Field dataField = Texture.class.getDeclaredField("data");
        unsafe().putObject(texture, unsafe().objectFieldOffset(dataField),
                Proxy.newProxyInstance(GremlinMatchGamePatchTest.class.getClassLoader(),
                        new Class<?>[]{TextureData.class},
                        (proxy, method, args) -> defaultValue(method.getReturnType())));
        for (Field field : com.megacrit.cardcrawl.helpers.ImageMaster.class.getDeclaredFields()) {
            field.setAccessible(true);
            if (field.getType() == Texture.class) {
                field.set(null, texture);
            } else if (field.getType() == com.badlogic.gdx.graphics.g2d.TextureAtlas.AtlasRegion.class) {
                com.badlogic.gdx.graphics.g2d.TextureAtlas.AtlasRegion region =
                        (com.badlogic.gdx.graphics.g2d.TextureAtlas.AtlasRegion)
                                unsafe().allocateInstance(com.badlogic.gdx.graphics.g2d.TextureAtlas.AtlasRegion.class);
                Field width = region.getClass().getField("packedWidth");
                Field height = region.getClass().getField("packedHeight");
                unsafe().putInt(region, unsafe().objectFieldOffset(width), 64);
                unsafe().putInt(region, unsafe().objectFieldOffset(height), 64);
                field.set(null, region);
            }
        }
    }

    private static Graphics graphicsWithDelta(float delta) {
        return (Graphics) Proxy.newProxyInstance(
                GremlinMatchGamePatchTest.class.getClassLoader(),
                new Class<?>[]{Graphics.class},
                (proxy, method, args) -> {
                    if (method.getName().equals("getDeltaTime")) return delta;
                    if (method.getName().equals("getDisplayMode")) return displayMode();
                    if (method.getReturnType() == Graphics.DisplayMode[].class) {
                        return new Graphics.DisplayMode[]{displayMode()};
                    }
                    return defaultValue(method.getReturnType());
                });
    }

    private static Graphics.DisplayMode displayMode() throws Exception {
        Graphics.DisplayMode mode = (Graphics.DisplayMode) unsafe().allocateInstance(Graphics.DisplayMode.class);
        for (String fieldName : new String[]{"width", "height", "refreshRate", "bitsPerPixel"}) {
            Field field = Graphics.DisplayMode.class.getField(fieldName);
            unsafe().putInt(mode, unsafe().objectFieldOffset(field),
                    fieldName.equals("width") ? 800 : fieldName.equals("height") ? 600
                            : fieldName.equals("refreshRate") ? 60 : 32);
        }
        return mode;
    }

    private static Object defaultValue(Class<?> type) {
        if (type == boolean.class) return false;
        if (type == int.class) return 0;
        if (type == long.class) return 0L;
        if (type == float.class) return 0.0F;
        if (type == double.class) return 0.0D;
        return null;
    }
}
