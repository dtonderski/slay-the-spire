package skrelpoid.superfastmode;

import com.megacrit.cardcrawl.actions.AbstractGameAction;

public final class SuperFastModeTimingTest {
    private static final class ProbeAction extends AbstractGameAction {
        ProbeAction(float initialDuration) {
            this.duration = initialDuration;
            this.startDuration = initialDuration;
        }

        @Override
        public void update() {}

        float duration() {
            return this.duration;
        }
    }

    public static void main(String[] args) {
        ProbeAction action = new ProbeAction(0.25F);
        SuperFastMode.tickGameplayDuration(action);
        if (action.isDone) {
            throw new AssertionError("a 0.25-second action expired on its opening gameplay tick");
        }
        float expected = 0.25F - (1.0F / 60.0F);
        if (Math.abs(action.duration() - expected) > 0.000001F) {
            throw new AssertionError("gameplay tick did not subtract exactly 1/60");
        }

        int ticks = 1;
        while (!action.isDone && ticks < 100) {
            SuperFastMode.tickGameplayDuration(action);
            ticks += 1;
        }
        if (!action.isDone || ticks != 16) {
            throw new AssertionError("0.25-second action should finish on tick 16, got " + ticks);
        }
    }
}
