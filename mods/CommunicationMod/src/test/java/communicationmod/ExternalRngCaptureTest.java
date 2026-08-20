package communicationmod;

import com.badlogic.gdx.math.MathUtils;
import com.badlogic.gdx.math.RandomXS128;
import com.megacrit.cardcrawl.rooms.AbstractRoom;
import org.junit.Test;

import java.util.ArrayList;
import java.util.HashMap;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

public class ExternalRngCaptureTest {
    public static void main(String[] args) {
        new ExternalRngCaptureTest().capturesExternalRngAndChecksBoundaryHelpers();
    }

    @Test
    public void capturesExternalRngAndChecksBoundaryHelpers() {
        RandomXS128 original = (RandomXS128) MathUtils.random;
        try {
            RandomXS128 rng = new RandomXS128(
                    0x0123456789ABCDEFL,
                    0xFEDCBA9876543210L
            );
            MathUtils.random = rng;
            ExternalRngCapture.clearPending();
            ExternalRngCapture.recordCardGroupGetRandomCardByType(16);

            assertEquals(0x0123456789ABCDEFL, rng.getState(0));
            assertEquals(0xFEDCBA9876543210L, rng.getState(1));
            ArrayList<HashMap<String, Object>> draws = ExternalRngCapture.drainPending();
            assertEquals(1, draws.size());
            HashMap<String, Object> draw = draws.get(0);
            assertEquals("card_group_get_random_card_by_type", draw.get("kind"));
            assertEquals(16, draw.get("range_inclusive"));
            @SuppressWarnings("unchecked")
            HashMap<String, Object> state = (HashMap<String, Object>) draw.get("state");
            assertEquals("0123456789abcdef", state.get("state0"));
            assertEquals("fedcba9876543210", state.get("state1"));
            assertTrue(ExternalRngCapture.drainPending().isEmpty());

            GameStateListener.resetStateVariables();
            assertEquals(6, GameStateListener.getBoundarySchema());
            assertFalse(GameStateListener.hasCompletingBoundary());
            long executionSeq = GameStateListener.getCommandExecutionSeq();
            GameStateListener.registerCommandExecution();
            assertEquals(executionSeq + 1L, GameStateListener.getCommandExecutionSeq());
            GameStateListener.resetStateVariables();
            assertEquals(executionSeq + 1L, GameStateListener.getCommandExecutionSeq());
            assertFalse(GameStateListener.hasCompletingBoundary());
            GameStateListener.registerStatePoll();
            assertTrue(GameStateListener.hasCompletingBoundary());
            assertEquals("poll", GameStateListener.consumeBoundaryKind());
            assertFalse(GameStateListener.hasCompletingBoundary());

            assertTrue(GameStateListener.isEndTurnUnresolved(
                    true,
                    true,
                    AbstractRoom.RoomPhase.COMBAT,
                    true
            ));
            assertFalse(GameStateListener.isEndTurnUnresolved(
                    true,
                    true,
                    AbstractRoom.RoomPhase.COMPLETE,
                    true
            ));
            assertFalse(GameStateListener.isEndTurnUnresolved(
                    true,
                    true,
                    AbstractRoom.RoomPhase.COMBAT,
                    false
            ));

            assertTrue(GameStateListener.retainDeferredOutOfCombatUpdate(true, false));
            assertFalse(GameStateListener.retainDeferredOutOfCombatUpdate(true, true));
            assertFalse(GameStateListener.retainDeferredOutOfCombatUpdate(false, false));

            assertTrue(GameStateListener.effectQueuesAreSettled(0, 0, 0));
            assertFalse(GameStateListener.effectQueuesAreSettled(1, 0, 0));
            assertFalse(GameStateListener.effectQueuesAreSettled(0, 1, 0));
            assertFalse(GameStateListener.effectQueuesAreSettled(0, 0, 1));

            assertTrue(GameStateListener.isQuiescentCombatBoundaryReady(false, true, true));
            assertFalse(GameStateListener.isQuiescentCombatBoundaryReady(true, true, true));
            assertFalse(GameStateListener.isQuiescentCombatBoundaryReady(false, false, true));
            assertFalse(GameStateListener.isQuiescentCombatBoundaryReady(false, true, false));
        } finally {
            MathUtils.random = original;
            ExternalRngCapture.clearPending();
        }
    }
}
