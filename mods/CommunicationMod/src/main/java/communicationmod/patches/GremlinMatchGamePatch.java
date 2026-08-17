package communicationmod.patches;

import basemod.ReflectionHacks;
import com.evacipated.cardcrawl.modthespire.lib.*;
import com.evacipated.cardcrawl.modthespire.patcher.PatchingException;
import com.megacrit.cardcrawl.cards.AbstractCard;
import com.megacrit.cardcrawl.cards.CardGroup;
import com.megacrit.cardcrawl.events.shrines.GremlinMatchGame;
import com.megacrit.cardcrawl.helpers.Hitbox;
import com.megacrit.cardcrawl.helpers.input.InputHelper;
import communicationmod.GameStateListener;
import javassist.CannotCompileException;
import javassist.CtBehavior;

import java.lang.reflect.Method;
import java.util.*;

public class GremlinMatchGamePatch {

    public static HashMap<UUID, Integer> cardPositions;
    public static CardGroup cards;
    public static Set<UUID> revealedCards;

    public static ArrayList<AbstractCard> getOrderedCards() {
        ArrayList<AbstractCard> returnedCards = new ArrayList<>(cards.group);
        returnedCards.sort(Comparator.comparingInt(c -> cardPositions.get(c.uuid)));
        returnedCards.removeIf(c -> !c.isFlipped);
        return returnedCards;
    }

    /**
     * Apply a Match and Keep face-down pick without waiting for a later
     * updateMatchGameLogic / Hitbox.update frame. The hover+justClickedLeft
     * fake needs that frame to land; SuperFastMode 100x and software GL can
     * skip it or freeze waitForEndTimer when getDeltaTime() is 0, leaving
     * CHOOSE accepted and never completing.
     */
    public static void chooseFaceDownCard(GremlinMatchGame event, int choiceIndex) {
        ArrayList<AbstractCard> pickable = getOrderedCards();
        AbstractCard card = pickable.get(choiceIndex);
        finishPendingMismatchWait(event);
        armCardClick(card);
        HoverCardPatch.hoverCard = card;
        HoverCardPatch.doHover = true;
        invokeUpdateMatchGameLogic(event);
        if (HoverCardPatch.doHover) {
            finishPendingMismatchWait(event);
            armCardClick(card);
            HoverCardPatch.hoverCard = card;
            HoverCardPatch.doHover = true;
            invokeUpdateMatchGameLogic(event);
        }
        HoverCardPatch.doHover = false;
        finishMatchGameIfDone(event);
        GameStateListener.registerStateChange();
    }

    /**
     * After the last pair (or when attempts are exhausted) vanilla waits on
     * {@code waitForEndTimer} then {@code waitTimer} before showing the leave
     * dialog. SuperFastMode / software GL can freeze those timers, so the
     * snapshot stays EVENT with an empty choice list and no proceed/leave.
     */
    public static void finishMatchGameIfDone(GremlinMatchGame event) {
        if (!shouldLeaveMatchGame(event)) {
            return;
        }
        setMatchWaitTimer(event, 0.0F);
        setWaitForEndTimer(event, 0.0F);
        invokeUpdateMatchGameLogic(event);
        setMatchWaitTimer(event, 0.0F);
        setWaitForEndTimer(event, 0.0F);
        invokeUpdateMatchGameLogic(event);
    }

    public static boolean shouldLeaveMatchGame(GremlinMatchGame event) {
        if (readBoolean(event, "gameDone")) {
            return true;
        }
        Integer attemptCount = readInteger(event, "attemptCount");
        return attemptCount != null && attemptCount <= 0 && getOrderedCards().isEmpty();
    }

    private static void armCardClick(AbstractCard card) {
        card.hb.hovered = true;
        card.hb.clickStarted = true;
        card.hb.clicked = true;
        InputHelper.justClickedLeft = true;
    }

    private static void finishPendingMismatchWait(GremlinMatchGame event) {
        // GremlinMatchGame.waitTimer is private and shadows AbstractEvent.waitTimer.
        setMatchWaitTimer(event, 0.0F);
        Float waitForEndTimer = readWaitForEndTimer(event);
        if (waitForEndTimer == null || waitForEndTimer <= 0.0F) {
            return;
        }
        AbstractCard chosen = (AbstractCard) ReflectionHacks.getPrivate(
                event, GremlinMatchGame.class, "chosenCard");
        AbstractCard hovered = (AbstractCard) ReflectionHacks.getPrivate(
                event, GremlinMatchGame.class, "hoveredCard");
        if (chosen != null) {
            chosen.isFlipped = true;
        }
        if (hovered != null) {
            hovered.isFlipped = true;
        }
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "chosenCard", null);
        ReflectionHacks.setPrivate(event, GremlinMatchGame.class, "hoveredCard", null);
        setWaitForEndTimer(event, 0.0F);
    }

    private static Float readWaitForEndTimer(GremlinMatchGame event) {
        return readFloat(event, "waitForEndTimer");
    }

    private static float readMatchWaitTimer(GremlinMatchGame event) {
        Float value = readFloat(event, "waitTimer");
        return value == null ? 0.0F : value;
    }

    private static void setMatchWaitTimer(GremlinMatchGame event, float value) {
        setPrivateFloat(event, "waitTimer", value);
    }

    private static void setWaitForEndTimer(GremlinMatchGame event, float value) {
        setPrivateFloat(event, "waitForEndTimer", value);
    }

    private static Float readFloat(GremlinMatchGame event, String field) {
        try {
            Object value = ReflectionHacks.getPrivate(event, GremlinMatchGame.class, field);
            return value instanceof Float ? (Float) value : null;
        } catch (RuntimeException ignored) {
            return null;
        }
    }

    private static Integer readInteger(GremlinMatchGame event, String field) {
        try {
            Object value = ReflectionHacks.getPrivate(event, GremlinMatchGame.class, field);
            return value instanceof Integer ? (Integer) value : null;
        } catch (RuntimeException ignored) {
            return null;
        }
    }

    private static boolean readBoolean(GremlinMatchGame event, String field) {
        try {
            Object value = ReflectionHacks.getPrivate(event, GremlinMatchGame.class, field);
            return value instanceof Boolean && (Boolean) value;
        } catch (RuntimeException ignored) {
            return false;
        }
    }

    private static void setPrivateFloat(GremlinMatchGame event, String field, float value) {
        try {
            ReflectionHacks.setPrivate(event, GremlinMatchGame.class, field, value);
        } catch (RuntimeException ignored) {
        }
    }

    private static void invokeUpdateMatchGameLogic(GremlinMatchGame event) {
        try {
            Method update = GremlinMatchGame.class.getDeclaredMethod("updateMatchGameLogic");
            update.setAccessible(true);
            update.invoke(event);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException("updateMatchGameLogic cannot be invoked", e);
        }
    }

    @SpirePatch(
            clz=GremlinMatchGame.class,
            method=SpirePatch.CONSTRUCTOR
    )
    public static class InitializeCardsPatch {

        public static void Postfix(GremlinMatchGame _instance) {
            cards = (CardGroup) ReflectionHacks.getPrivate(_instance, GremlinMatchGame.class, "cards");
            revealedCards = new HashSet<>();
            // If 0 is top left and 11 is bottom right, the positions of the cards in the result array are:
            // [0, 5, 10, 3, 4, 9, 2, 7, 8, 1, 6, 11]. We want to store the initial positions for easy reference.
            // Cards can be removed from the card group, so it is easier to just calculate them at the start.
            cardPositions = new HashMap<>();
            for(int i = 0; i < 12; i++) {
                AbstractCard currentCard = cards.group.get(i);
                int target_x = i % 4;
                int target_y = i % 3;
                int position = target_x + 4 * target_y;
                cardPositions.put(currentCard.uuid, position);
            }
        }
    }

    @SpirePatch(
            clz=GremlinMatchGame.class,
            method="updateMatchGameLogic"
    )
    public static class HoverCardPatch {

        public static boolean doHover = false;
        public static AbstractCard hoverCard = null;

        @SpireInsertPatch(
                locator=Locator.class,
                localvars = {"c"}
        )
        public static void Insert(GremlinMatchGame _instance, AbstractCard c) {
            if (!doHover) {
                return;
            }
            // The match minigame ignores clicks while the previous pair is still
            // flipping. Consuming doHover in that window leaves CommunicationMod
            // waiting for a completing boundary that never arrives.
            if (readMatchWaitTimer(_instance) > 0.0F) {
                return;
            }
            Float waitForEndTimer = readWaitForEndTimer(_instance);
            if (waitForEndTimer != null && waitForEndTimer > 0.0F) {
                return;
            }
            if (c.equals(hoverCard)) {
                c.hb.hovered = true;
                c.hb.clickStarted = true;
                c.hb.clicked = true;
                InputHelper.justClickedLeft = true;
                doHover = false;
            } else {
                c.hb.hovered = false;
            }
        }

        private static class Locator extends SpireInsertLocator {
            public int[] Locate(CtBehavior ctMethodToPatch) throws CannotCompileException, PatchingException {
                Matcher matcher = new Matcher.MethodCallMatcher(Hitbox.class, "update");
                int[] result = LineFinder.findInOrder(ctMethodToPatch, new ArrayList<Matcher>(), matcher);
                result[0] += 1;
                return result;
            }
        }


    }

    @SpirePatch(
            clz=GremlinMatchGame.class,
            method="updateMatchGameLogic"
    )
    public static class WaitForCardFlipPatch {

        @SpireInsertPatch(
                locator=Locator.class
        )
        public static void Insert(GremlinMatchGame _instance) {
            // We have to wait for the cards to flip or everything goes wrong. Nothing else detects this change.
            int attemptCount = (int) ReflectionHacks.getPrivate(_instance, GremlinMatchGame.class, "attemptCount");
            if(attemptCount > 0) {
                GameStateListener.registerStateChange();
            }
        }

        private static class Locator extends SpireInsertLocator {
            public int[] Locate(CtBehavior ctMethodToPatch) throws CannotCompileException, PatchingException {
                Matcher matcher = new Matcher.FieldAccessMatcher(GremlinMatchGame.class, "attemptCount");
                int[] result = LineFinder.findInOrder(ctMethodToPatch, new ArrayList<Matcher>(), matcher);
                result[0] += 1;
                return result;
            }
        }

    }

    @SpirePatch(
            clz=GremlinMatchGame.class,
            method="updateMatchGameLogic"
    )
    public static class CardIdentificationPatch {

        @SpireInsertPatch(
                locator=Locator.class,
                localvars = {"c"}
        )
        public static void Insert(GremlinMatchGame _instance, AbstractCard c) {
            revealedCards.add(c.uuid);
        }

        private static class Locator extends SpireInsertLocator {
            public int[] Locate(CtBehavior ctMethodToPatch) throws CannotCompileException, PatchingException {
                Matcher matcher = new Matcher.FieldAccessMatcher(AbstractCard.class, "isFlipped");
                int[] matches = LineFinder.findAllInOrder(ctMethodToPatch, new ArrayList<Matcher>(), matcher);
                return Arrays.copyOfRange(matches, 1, 2);
            }
        }
    }

    @SpirePatch(
            clz=GremlinMatchGame.class,
            method="updateMatchGameLogic"
    )
    public static class RegisterFirstFlipPatch{

        @SpireInsertPatch(
                locator=Locator.class
        )
        public static void Insert(GremlinMatchGame _instance) {
            GameStateListener.registerStateChange();
        }

        /*
            This locator tries to find the line this.chosenCard = this.hoveredCard, which indicates the first flip of a match
         */
        private static class Locator extends SpireInsertLocator {
            public int[] Locate(CtBehavior ctMethodToPatch) throws CannotCompileException, PatchingException {
                Matcher chosenMatcher = new Matcher.FieldAccessMatcher(GremlinMatchGame.class, "chosenCard");
                Matcher hoveredMatcher = new Matcher.FieldAccessMatcher(GremlinMatchGame.class, "hoveredCard");
                int[] chosenMatches = LineFinder.findAllInOrder(ctMethodToPatch, new ArrayList<Matcher>(), chosenMatcher);
                int[] hoveredMatches = LineFinder.findAllInOrder(ctMethodToPatch, new ArrayList<Matcher>(), hoveredMatcher);
                // Not the most computationally efficient way to do this, but it should only be run once anyway.
                for (int waitMatch : chosenMatches) {
                    for (int gameDoneMatch : hoveredMatches) {
                        if (waitMatch == gameDoneMatch) {
                            int[] match = new int[1];
                            match[0] = waitMatch;
                            return match;
                        }
                    }
                }
                throw new PatchingException("Could not find patching location for RegisterFirstFlipPatch in GremlinMatchGame.");
            }
        }

    }
}
