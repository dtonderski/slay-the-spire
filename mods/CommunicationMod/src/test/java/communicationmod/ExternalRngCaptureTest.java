package communicationmod;

import com.badlogic.gdx.math.MathUtils;
import com.badlogic.gdx.math.RandomXS128;
import com.megacrit.cardcrawl.rooms.AbstractRoom;

import java.util.ArrayList;
import java.util.HashMap;

public class ExternalRngCaptureTest {
    public static void main(String[] args) {
        RandomXS128 original = (RandomXS128) MathUtils.random;
        try {
            RandomXS128 rng = new RandomXS128(
                    0x0123456789ABCDEFL,
                    0xFEDCBA9876543210L
            );
            MathUtils.random = rng;
            ExternalRngCapture.clearPending();
            ExternalRngCapture.recordCardGroupGetRandomCardByType(16);

            assert rng.getState(0) == 0x0123456789ABCDEFL;
            assert rng.getState(1) == 0xFEDCBA9876543210L;
            ArrayList<HashMap<String, Object>> draws = ExternalRngCapture.drainPending();
            assert draws.size() == 1;
            HashMap<String, Object> draw = draws.get(0);
            assert draw.get("kind").equals("card_group_get_random_card_by_type");
            assert draw.get("range_inclusive").equals(16);
            @SuppressWarnings("unchecked")
            HashMap<String, Object> state = (HashMap<String, Object>) draw.get("state");
            assert state.get("state0").equals("0123456789abcdef");
            assert state.get("state1").equals("fedcba9876543210");
            assert ExternalRngCapture.drainPending().isEmpty();

            GameStateListener.resetStateVariables();
            assert !GameStateListener.hasCompletingBoundary();
            GameStateListener.registerCommandExecution();
            assert !GameStateListener.hasCompletingBoundary();
            GameStateListener.registerStatePoll();
            assert GameStateListener.hasCompletingBoundary();
            assert GameStateListener.consumeBoundaryKind().equals("poll");
            assert !GameStateListener.hasCompletingBoundary();

            assert GameStateListener.isEndTurnUnresolved(
                    true,
                    true,
                    AbstractRoom.RoomPhase.COMBAT,
                    true
            );
            assert !GameStateListener.isEndTurnUnresolved(
                    true,
                    true,
                    AbstractRoom.RoomPhase.COMPLETE,
                    true
            );
            assert !GameStateListener.isEndTurnUnresolved(
                    true,
                    true,
                    AbstractRoom.RoomPhase.COMBAT,
                    false
            );
        } finally {
            MathUtils.random = original;
            ExternalRngCapture.clearPending();
        }
    }
}
