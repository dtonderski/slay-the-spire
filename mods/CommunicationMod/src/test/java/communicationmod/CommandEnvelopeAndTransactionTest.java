package communicationmod;

import org.junit.Test;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

public class CommandEnvelopeAndTransactionTest {
    @Test
    public void parsesJsonEnvelopeAndLegacyText() throws Exception {
        CommandEnvelope envelope = CommandEnvelope.parse(
                "{\"command_id\":\"cmd-1\",\"command\":\"CHOOSE 0\"}"
        );
        assertEquals("cmd-1", envelope.commandId);
        assertEquals("CHOOSE 0", envelope.command);
        assertEquals("choose", envelope.verb());

        CommandEnvelope legacy = CommandEnvelope.parse("END");
        assertNull(legacy.commandId);
        assertEquals("END", legacy.command);
        assertEquals("end", legacy.verb());
        assertTrue(CommandEnvelope.isObservationVerb("state"));
        assertTrue(CommandEnvelope.isObservationVerb("profile"));
        assertFalse(CommandEnvelope.isObservationVerb("end"));
    }

    @Test
    public void rejectsMalformedEnvelopes() {
        String[] invalid = {
                "{",
                "{\"command\":\"END\"}",
                "{\"command_id\":\"\",\"command\":\"END\"}",
                "{\"command_id\":\"cmd-1\",\"command\":\"\"}",
                "{\"command_id\":\"cmd-1\",\"command\":\"END\",\"extra\":true}",
        };
        for (String raw : invalid) {
            try {
                CommandEnvelope.parse(raw);
                fail("expected invalid envelope: " + raw);
            } catch (InvalidCommandException ignored) {
            }
        }
    }

    @Test
    public void gameplayPollAndRejectionAdvanceExactSequences() {
        GameStateListener.resetStateVariables();
        assertEquals(7, GameStateListener.getBoundarySchema());
        long execution = GameStateListener.getCommandExecutionSeq();
        long settlement = GameStateListener.getCommandSettlementSeq();

        GameStateListener.beforeCommand("start-1", "start");
        // START resets run-state detectors synchronously; it must not erase the
        // already-owned command transaction.
        GameStateListener.resetStateVariables();
        GameStateListener.afterCommand("start-1", "start", true);
        assertEquals(execution + 1L, GameStateListener.getCommandExecutionSeq());
        assertEquals(settlement, GameStateListener.getCommandSettlementSeq());
        assertTrue(GameStateListener.isTransactionPending());
        assertEquals("unsolicited", GameStateListener.getCommandResponseKind());
        GameStateListener.stampPublishedGameplayResponse();
        assertEquals(execution + 1L, GameStateListener.getCommandExecutionSeq());
        assertEquals(settlement + 1L, GameStateListener.getCommandSettlementSeq());
        assertEquals("start-1", GameStateListener.getCommandResponseId());
        assertEquals("settled", GameStateListener.getCommandResponseKind());
        assertFalse(GameStateListener.isTransactionPending());

        GameStateListener.resetStateVariables();
        execution = GameStateListener.getCommandExecutionSeq();
        settlement = GameStateListener.getCommandSettlementSeq();
        GameStateListener.beforeCommand("bad-1", "choose");
        GameStateListener.afterRejectedCommand("bad-1", "choose");
        assertEquals(execution + 1L, GameStateListener.getCommandExecutionSeq());
        assertEquals(settlement, GameStateListener.getCommandSettlementSeq());
        assertEquals("bad-1", GameStateListener.getCommandResponseId());
        assertEquals("rejected", GameStateListener.getCommandResponseKind());
        assertFalse(GameStateListener.isTransactionPending());

        execution = GameStateListener.getCommandExecutionSeq();
        settlement = GameStateListener.getCommandSettlementSeq();
        GameStateListener.beforeCommand("state-1", "state");
        GameStateListener.afterCommand("state-1", "state", false);
        assertEquals(execution, GameStateListener.getCommandExecutionSeq());
        assertEquals(settlement, GameStateListener.getCommandSettlementSeq());
        assertTrue(GameStateListener.hasCompletingBoundary());
        assertEquals("poll", GameStateListener.consumeBoundaryKind());
        assertEquals("state-1", GameStateListener.getCommandResponseId());
        assertEquals("poll", GameStateListener.getCommandResponseKind());
        assertEquals(execution, GameStateListener.getCommandExecutionSeq());
        assertEquals(settlement, GameStateListener.getCommandSettlementSeq());
        assertFalse(GameStateListener.hasCompletingBoundary());

        GameStateListener.beforeCommand("profile-1", "profile");
        GameStateListener.afterCommand("profile-1", "profile", false);
        assertEquals(execution, GameStateListener.getCommandExecutionSeq());
        assertEquals(settlement, GameStateListener.getCommandSettlementSeq());

        GameStateListener.beforeCommand("profile-bad", "profile");
        GameStateListener.afterRejectedCommand("profile-bad", "profile");
        assertEquals(execution, GameStateListener.getCommandExecutionSeq());
        assertEquals(settlement, GameStateListener.getCommandSettlementSeq());
        assertEquals("profile-bad", GameStateListener.getCommandResponseId());
        assertEquals("rejected", GameStateListener.getCommandResponseKind());

        GameStateListener.afterUnidentifiedRejectedCommand();
        assertNull(GameStateListener.getCommandResponseId());
        assertEquals("rejected", GameStateListener.getCommandResponseKind());
    }
}
