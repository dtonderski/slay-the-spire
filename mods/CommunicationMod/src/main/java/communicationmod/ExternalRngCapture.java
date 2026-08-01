package communicationmod;

import com.badlogic.gdx.math.MathUtils;
import com.badlogic.gdx.math.RandomXS128;

import java.util.ArrayList;
import java.util.HashMap;

/**
 * One-shot gameplay inputs captured from process-global RNG.
 *
 * Values are drained into the next CommunicationMod response. State words are
 * strings because the Node trace bridge cannot represent arbitrary Java longs
 * losslessly as JSON numbers.
 */
public final class ExternalRngCapture {
    private static final ArrayList<HashMap<String, Object>> pending = new ArrayList<>();

    private ExternalRngCapture() {}

    public static synchronized void recordCardGroupGetRandomCardByType(int rangeInclusive) {
        if (!(MathUtils.random instanceof RandomXS128)) {
            throw new IllegalStateException("MathUtils.random is not RandomXS128");
        }
        RandomXS128 rng = (RandomXS128) MathUtils.random;
        HashMap<String, Object> state = new HashMap<>();
        state.put("state0", formatStateWord(rng.getState(0)));
        state.put("state1", formatStateWord(rng.getState(1)));

        HashMap<String, Object> draw = new HashMap<>();
        draw.put("kind", "card_group_get_random_card_by_type");
        draw.put("state", state);
        draw.put("range_inclusive", rangeInclusive);
        pending.add(draw);
    }

    public static synchronized ArrayList<HashMap<String, Object>> drainPending() {
        ArrayList<HashMap<String, Object>> drained = new ArrayList<>(pending);
        pending.clear();
        return drained;
    }

    public static synchronized void clearPending() {
        pending.clear();
    }

    static String formatStateWord(long value) {
        return String.format("%016x", value);
    }
}
