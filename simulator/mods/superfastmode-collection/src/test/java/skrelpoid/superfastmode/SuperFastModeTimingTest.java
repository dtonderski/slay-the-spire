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
        boolean imageEventRaw = false;
        boolean showCardObtainRaw = false;
        boolean exhaustCardRaw = false;
        boolean smithRaw = false;
        boolean tokeRaw = false;
        boolean campfireRaw = false;
        for (com.evacipated.cardcrawl.modthespire.lib.SpirePatch patch :
                skrelpoid.superfastmode.patches.DefaultDeltaPatches.DeltaPatch.class
                        .getAnnotationsByType(com.evacipated.cardcrawl.modthespire.lib.SpirePatch.class)) {
            if (!patch.method().equals("update")) continue;
            imageEventRaw |= patch.clz().getName().equals("com.megacrit.cardcrawl.events.AbstractImageEvent");
            showCardObtainRaw |= patch.clz().getName().equals("com.megacrit.cardcrawl.vfx.cardManip.ShowCardAndObtainEffect");
            exhaustCardRaw |= patch.clz().getName().equals("com.megacrit.cardcrawl.vfx.cardManip.ExhaustCardEffect");
            smithRaw |= patch.clz().getName().equals("com.megacrit.cardcrawl.vfx.campfire.CampfireSmithEffect");
            tokeRaw |= patch.clz().getName().equals("com.megacrit.cardcrawl.vfx.campfire.CampfireTokeEffect");
            campfireRaw |= patch.clz().getName().equals("com.megacrit.cardcrawl.rooms.CampfireUI");
        }
        if (!imageEventRaw) {
            throw new AssertionError("image-event dialog timers need raw delta to preserve their strict-negative show transition");
        }
        if (!showCardObtainRaw) {
            throw new AssertionError("card-obtain effects need raw delta so Omamori-blocked curses remain done before the obtain threshold");
        }
        if (!exhaustCardRaw) {
            throw new AssertionError("exhaust effects reset card attributes and must use the raw target clock");
        }
        if (!smithRaw || !tokeRaw || campfireRaw) {
            throw new AssertionError("Smith/Toke need raw delta; shared UI hide timing must remain aligned with other options");
        }
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
