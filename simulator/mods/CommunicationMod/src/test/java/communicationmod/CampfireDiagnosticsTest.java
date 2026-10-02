package communicationmod;

import com.google.gson.JsonParser;
import com.megacrit.cardcrawl.vfx.AbstractGameEffect;
import org.junit.After;
import org.junit.Test;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

public class CampfireDiagnosticsTest {
    private final String previousPath = System.getProperty(CampfireDiagnostics.PATH_PROPERTY);
    private final String previousCap = System.getProperty(CampfireDiagnostics.CAP_PROPERTY);
    private final String previousTail = System.getProperty(CampfireDiagnostics.TAIL_PROPERTY);
    private final String previousMinCommand = System.getProperty(CampfireDiagnostics.MIN_COMMAND_PROPERTY);

    @After
    public void cleanup() {
        restore(CampfireDiagnostics.PATH_PROPERTY, previousPath);
        restore(CampfireDiagnostics.CAP_PROPERTY, previousCap);
        restore(CampfireDiagnostics.TAIL_PROPERTY, previousTail);
        restore(CampfireDiagnostics.MIN_COMMAND_PROPERTY, previousMinCommand);
        CampfireDiagnostics.resetForTests();
    }

    @Test
    public void disabledByDefaultDoesNotOpenOrRecord() {
        System.clearProperty(CampfireDiagnostics.PATH_PROPERTY);
        CampfireDiagnostics.resetForTests();
        CampfireDiagnostics.commandEntry("com.megacrit.cardcrawl.ui.campfire.RecallOption");
        assertEquals(0, CampfireDiagnostics.recordCountForTests());
        assertEquals(null, CampfireDiagnostics.outputPathForTests());
    }

    @Test
    public void enabledOutputIsBoundedAndCollisionSafe() throws Exception {
        Path directory = Files.createTempDirectory("campfire-diagnostics");
        Path requested = directory.resolve("window.jsonl");
        Files.write(requested, "prior\n".getBytes(StandardCharsets.UTF_8));
        System.setProperty(CampfireDiagnostics.PATH_PROPERTY, requested.toString());
        System.setProperty(CampfireDiagnostics.CAP_PROPERTY, "3");
        CampfireDiagnostics.resetForTests();

        CampfireDiagnostics.commandEntry("com.megacrit.cardcrawl.ui.campfire.DigOption");
        CampfireDiagnostics.dungeonUpdate("dungeon_update_post");
        CampfireDiagnostics.commandEntry("com.megacrit.cardcrawl.ui.campfire.DigOption");

        String actual = CampfireDiagnostics.outputPathForTests();
        assertNotNull(actual);
        assertTrue(actual.endsWith("window.jsonl.1"));
        List<String> lines = Files.readAllLines(Paths.get(actual), StandardCharsets.UTF_8);
        assertEquals(3, lines.size());
        assertTrue(lines.get(0).contains("command_entry"));
        assertTrue(lines.get(1).contains("dungeon_update_post"));
        assertTrue(new JsonParser().parse(lines.get(2)).getAsJsonObject().get("truncated").getAsBoolean());
        assertEquals("max_records", new JsonParser().parse(lines.get(2)).getAsJsonObject().get("reason").getAsString());
        assertEquals("command_entry", new JsonParser().parse(lines.get(0)).getAsJsonObject().get("stage").getAsString());
        assertEquals("dungeon_update_post", new JsonParser().parse(lines.get(1)).getAsJsonObject().get("stage").getAsString());
        assertEquals("prior\n", new String(Files.readAllBytes(requested), StandardCharsets.UTF_8));
    }

    @Test
    public void pairedCallbacksCountOnlyDungeonPostAndMarkTruncation() throws Exception {
        Path directory = Files.createTempDirectory("campfire-diagnostics-tail");
        System.setProperty(CampfireDiagnostics.PATH_PROPERTY, directory.resolve("window.jsonl").toString());
        System.setProperty(CampfireDiagnostics.CAP_PROPERTY, "64");
        System.setProperty(CampfireDiagnostics.TAIL_PROPERTY, "8");
        CampfireDiagnostics.resetForTests();

        GameStateListener.resetStateVariables();
        GameStateListener.beforeCommand("card-1", "choose");
        CampfireDiagnostics.cardRewardCommandEntry();
        GameStateListener.stampPublishedGameplayResponse();
        for (int i = 0; i < 8; i++) {
            GameStateListener.signalGameUpdate();
            GameStateListener.signalDungeonUpdate();
        }

        List<String> lines = Files.readAllLines(
                Paths.get(CampfireDiagnostics.outputPathForTests()), StandardCharsets.UTF_8);
        long postUpdates = lines.stream().filter(line -> line.contains("dungeon_update_post")).count();
        assertEquals(8L, postUpdates);
        assertTrue(lines.get(lines.size() - 1).contains("\"truncated\":true"));
        assertEquals("tail_updates", new JsonParser().parse(lines.get(lines.size() - 1)).getAsJsonObject().get("reason").getAsString());
    }

    @Test
    public void publicationDoesNotHideDelayedEffectAndClosesAfterNaturalObservation() throws Exception {
        Path directory = Files.createTempDirectory("campfire-diagnostics-lifecycle");
        System.setProperty(CampfireDiagnostics.PATH_PROPERTY, directory.resolve("window.jsonl").toString());
        System.setProperty(CampfireDiagnostics.CAP_PROPERTY, "64");
        System.setProperty(CampfireDiagnostics.TAIL_PROPERTY, "4");
        CampfireDiagnostics.resetForTests();
        GameStateListener.resetStateVariables();
        GameStateListener.beforeCommand("recall-1", "choose");

        CampfireDiagnostics.commandEntry("com.megacrit.cardcrawl.ui.campfire.RecallOption");
        GameStateListener.stampPublishedGameplayResponse();
        GameStateListener.signalGameUpdate();
        GameStateListener.signalDungeonUpdate();
        CampfireDiagnostics.effectUpdateEntered("ObtainKeyEffect", null);
        CampfireDiagnostics.effectUpdateExited("ObtainKeyEffect", null);
        GameStateListener.signalGameUpdate();
        GameStateListener.signalDungeonUpdate();
        int completedCount = CampfireDiagnostics.recordCountForTests();
        GameStateListener.signalGameUpdate();

        List<String> lines = Files.readAllLines(
                Paths.get(CampfireDiagnostics.outputPathForTests()), StandardCharsets.UTF_8);
        assertTrue(lines.stream().anyMatch(line -> line.contains("state_publish")));
        assertTrue(lines.stream().anyMatch(line -> line.contains("ObtainKeyEffect")));
        assertEquals(completedCount, CampfireDiagnostics.recordCountForTests());
    }

    @Test
    public void observerDoesNotCreateExternalRngInputs() throws Exception {
        ExternalRngCapture.clearPending();
        Path directory = Files.createTempDirectory("campfire-diagnostics-rng");
        System.setProperty(CampfireDiagnostics.PATH_PROPERTY, directory.resolve("window.jsonl").toString());
        CampfireDiagnostics.resetForTests();
        CampfireDiagnostics.commandEntry("com.megacrit.cardcrawl.ui.campfire.DigOption");
        GameStateListener.signalGameUpdate();
        GameStateListener.signalDungeonUpdate();
        assertTrue(ExternalRngCapture.drainPending().isEmpty());
    }

    @Test
    public void fileFailureDisablesWithoutEscapingIntoUpdateCallback() throws Exception {
        Path directory = Files.createTempDirectory("campfire-diagnostics-failure");
        Path parentFile = directory.resolve("not-a-directory");
        Files.write(parentFile, "occupied".getBytes(StandardCharsets.UTF_8));
        System.setProperty(CampfireDiagnostics.PATH_PROPERTY, parentFile.resolve("window.jsonl").toString());
        CampfireDiagnostics.resetForTests();
        CampfireDiagnostics.commandEntry("com.megacrit.cardcrawl.ui.campfire.DigOption");
        GameStateListener.signalGameUpdate();
        GameStateListener.signalDungeonUpdate();
        assertEquals(null, CampfireDiagnostics.outputPathForTests());
        assertEquals(0, CampfireDiagnostics.recordCountForTests());
    }

    @Test
    public void effectSnapshotsSerializeStartingDuration() throws Exception {
        Path directory = Files.createTempDirectory("campfire-diagnostics-duration");
        System.setProperty(CampfireDiagnostics.PATH_PROPERTY, directory.resolve("window.jsonl").toString());
        CampfireDiagnostics.resetForTests();
        CampfireDiagnostics.commandEntry("com.megacrit.cardcrawl.ui.campfire.RecallOption");
        ProbeEffect effect = new ProbeEffect();
        effect.startingDuration = 2.0F;
        effect.duration = 1.25F;
        CampfireDiagnostics.effectUpdateEntered("CampfireRecallEffect", effect);
        JsonParser parser = new JsonParser();
        List<String> lines = Files.readAllLines(
                Paths.get(CampfireDiagnostics.outputPathForTests()), StandardCharsets.UTF_8);
        assertEquals("2.0", parser.parse(lines.get(lines.size() - 1)).getAsJsonObject()
                .get("starting_duration").getAsString());
    }

    @Test
    public void smithWindowHonorsThresholdAndDoesNotMutateEffect() throws Exception {
        Path directory = Files.createTempDirectory("smith-diagnostics");
        System.setProperty(CampfireDiagnostics.PATH_PROPERTY, directory.resolve("window.jsonl").toString());
        System.setProperty(CampfireDiagnostics.MIN_COMMAND_PROPERTY,
                Long.toString(GameStateListener.getCommandExecutionSeq() + 1L));
        CampfireDiagnostics.resetForTests();
        CampfireDiagnostics.commandEntry("SmithOption");
        assertEquals(null, CampfireDiagnostics.outputPathForTests());
        System.setProperty(CampfireDiagnostics.MIN_COMMAND_PROPERTY, "0");
        CampfireDiagnostics.commandEntry("SmithOption");
        ProbeEffect effect = new ProbeEffect();
        effect.duration = 0.125f;
        effect.isDone = false;
        CampfireDiagnostics.effectUpdateEntered("CampfireSmithEffect", effect);
        CampfireDiagnostics.effectUpdateExited("CampfireSmithEffect", effect);
        assertEquals(0.125f, effect.duration, 0.0f);
        assertEquals(false, effect.isDone);
        List<String> lines = Files.readAllLines(Paths.get(CampfireDiagnostics.outputPathForTests()), StandardCharsets.UTF_8);
        assertTrue(lines.stream().anyMatch(line -> line.contains("SmithOption")));
        assertTrue(lines.stream().anyMatch(line -> line.contains("CampfireSmithEffect")));
    }

    private static final class ProbeEffect extends AbstractGameEffect {
        @Override
        public void render(com.badlogic.gdx.graphics.g2d.SpriteBatch spriteBatch) { }

        @Override
        public void dispose() { }
    }

    @Test
    public void relativePathIsRejectedByDocumentedContract() throws Exception {
        System.setProperty(CampfireDiagnostics.PATH_PROPERTY, "relative-window.jsonl");
        CampfireDiagnostics.resetForTests();
        CampfireDiagnostics.commandEntry("com.megacrit.cardcrawl.ui.campfire.DigOption");
        assertEquals(null, CampfireDiagnostics.outputPathForTests());
        assertEquals(0, CampfireDiagnostics.recordCountForTests());
    }

    private static void restore(String key, String value) {
        if (value == null) System.clearProperty(key);
        else System.setProperty(key, value);
    }
}
