package communicationmod;

import com.megacrit.cardcrawl.actions.AbstractGameAction;
import com.megacrit.cardcrawl.actions.GameActionManager;
import com.megacrit.cardcrawl.core.CardCrawlGame;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.neow.NeowRoom;
import com.megacrit.cardcrawl.rooms.AbstractRoom;
import com.megacrit.cardcrawl.rooms.EventRoom;
import com.megacrit.cardcrawl.rooms.VictoryRoom;

public class GameStateListener {
    private static AbstractDungeon.CurrentScreen previousScreen = null;
    private static boolean previousScreenUp = false;
    private static AbstractRoom.RoomPhase previousPhase = null;
    private static boolean previousGridSelectConfirmUp = false;
    private static int previousGold = 99;
    private static boolean externalChange = false;
    private static boolean myTurn = false;
    private static boolean blocked = false;
    private static boolean waitingForCommand = false;
    private static boolean hasPresentedOutOfGameState = false;
    private static boolean waitOneUpdate = false;
    private static int timeout = 0;
    // 1: original readiness behaviour.
    // 2: readiness no longer publishes during a queued end turn, and the
    //    payload carries end_turn_queued. Traces are not comparable across
    //    this boundary: a v1 trace can contain commands accepted mid-turn.
    private static final int BOUNDARY_SCHEMA = 2;
    private static String boundaryKind = "unknown";
    private static boolean pollPending = false;
    private static long gameUpdateSeq = 0L;
    private static long dungeonUpdateSeq = 0L;
    private static AbstractGameAction trackedAction = null;
    private static long currentActionInstance = 0L;
    private static long currentActionUpdateCount = 0L;

    /**
     * Used to indicate that something (in game logic, not external command) has been done that will change the game state,
     * and hasStateChanged() should indicate a state change when the state next becomes stable.
     */
    public static void registerStateChange() {
        externalChange = true;
        waitingForCommand = false;
    }

    /**
     * Used to tell hasStateChanged() to indicate a state change after a specified number of frames.
     * @param newTimeout The number of frames to wait
     */
    public static void setTimeout(int newTimeout) {
        timeout = newTimeout;
    }

    /**
     * Used to indicate that an external command has been executed
     */
    public static void registerCommandExecution() {
        waitingForCommand = false;
        boundaryKind = "unknown";
        pollPending = false;
    }

    /** Marks an explicit STATE response, which is observation rather than gameplay settlement. */
    public static void registerStatePoll() {
        pollPending = true;
    }

    /** Counts one process update without changing target game behavior. */
    public static void signalGameUpdate() {
        gameUpdateSeq += 1L;
    }

    /** Counts one dungeon update without changing target game behavior. */
    public static void signalDungeonUpdate() {
        dungeonUpdateSeq += 1L;
    }

    /**
     * Records an invocation of the current action's update method. The
     * GameActionManager patch calls this only on the bytecode path that invokes
     * currentAction.update().
     */
    public static void signalCurrentActionUpdate(AbstractGameAction action) {
        observeCurrentAction(action);
        if (action != null) {
            currentActionUpdateCount += 1L;
        }
    }

    private static void observeCurrentAction(AbstractGameAction action) {
        if (action != trackedAction) {
            trackedAction = action;
            if (action != null) {
                currentActionInstance += 1L;
            }
            currentActionUpdateCount = 0L;
        }
    }

    /**
     * Prevents hasStateChanged() from indicating a state change until resumeStateUpdate() is called.
     */
    public static void blockStateUpdate() {
        blocked = true;
    }

    /**
     * Removes the block instantiated by blockStateChanged()
     */
    public static void resumeStateUpdate() {
        blocked = false;
    }

    /**
     * Used by a patch in the game to signal the start of your turn. We do not care about state changes
     * when it is not our turn in combat, as we cannot take action until then.
     */
    public static void signalTurnStart() {
        myTurn = true;
    }

    /**
     * Used by patches in the game to signal the end of your turn (or the end of combat).
     */
    public static void signalTurnEnd() {
        myTurn = false;
    }

    /**
     * Resets all state detection variables for the start of a new run.
     */
    public static void resetStateVariables() {
        ExternalRngCapture.clearPending();
        previousScreen = null;
        previousScreenUp = false;
        previousPhase = null;
        previousGridSelectConfirmUp = false;
        previousGold = 99;
        externalChange = false;
        myTurn = false;
        blocked = false;
        waitingForCommand = false;
        waitOneUpdate = false;
        boundaryKind = "unknown";
        pollPending = false;
        trackedAction = null;
        currentActionUpdateCount = 0L;
    }

    /**
     * Detects whether the game state is stable and we are ready to receive a command from the user.
     *
     * @return whether the state is stable
     */
    private static boolean hasDungeonStateChanged() {
        if (blocked) {
            return false;
        }
        hasPresentedOutOfGameState = false;
        AbstractDungeon.CurrentScreen newScreen = AbstractDungeon.screen;
        boolean newScreenUp = AbstractDungeon.isScreenUp;
        AbstractRoom.RoomPhase newPhase = AbstractDungeon.getCurrRoom().phase;
        boolean inCombat = (newPhase == AbstractRoom.RoomPhase.COMBAT);
        // Lots of stuff can happen while the dungeon is fading out, but nothing that requires input from the user.
        if (AbstractDungeon.isFadingOut || AbstractDungeon.isFadingIn) {
            return false;
        }
        // This check happens before the rest since dying can happen in combat and messes with the other cases.
        if (newScreen == AbstractDungeon.CurrentScreen.DEATH && newScreen != previousScreen) {
            return true;
        }
        // These screens have no interaction available.
        if (newScreen == AbstractDungeon.CurrentScreen.DOOR_UNLOCK || newScreen == AbstractDungeon.CurrentScreen.NO_INTERACT) {
            return false;
        }
        // We are not ready to receive commands when it is not our turn, except for some pesky screens
        if (inCombat && (!myTurn || AbstractDungeon.getMonsters().areMonstersBasicallyDead())) {
            if (!newScreenUp) {
                return false;
            }
        }
        // In event rooms, we need to wait for the event wait timer to reach 0 before we can accurately assess its state.
        AbstractRoom currentRoom = AbstractDungeon.getCurrRoom();
        if ((currentRoom instanceof EventRoom
                || currentRoom instanceof NeowRoom
                || (currentRoom instanceof VictoryRoom && ((VictoryRoom) currentRoom).eType == VictoryRoom.EventType.HEART))
                && AbstractDungeon.getCurrRoom().event.waitTimer != 0.0F) {
            return false;
        }
        // The state has always changed in some way when one of these variables is different.
        // However, the state may not be finished changing, so we need to do some additional checks.
        if (newScreen != previousScreen || newScreenUp != previousScreenUp || newPhase != previousPhase) {
            if (inCombat) {
                // In combat, newScreenUp being true indicates an action that requires our immediate attention.
                if (newScreenUp) {
                    return true;
                }
                // In combat, if no screen is up, we should wait for all actions to complete before indicating a state change.
                //
                // The action queue can be transiently empty in the middle of an
                // end turn: the EndTurnAction has already been popped and its
                // follow-up actions are not queued yet. Reporting ready in that
                // window lets the next command land on an unfinished turn, which
                // produces a second EndTurnAction and can drop a pending screen
                // selection entirely. endTurnQueued stays true until the turn
                // actually ends, so it distinguishes that window from a settled
                // boundary. The same guard exists below at the externalChange
                // case; this branch returns before reaching it.
                else if (!isEndTurnStillResolving()
                        && AbstractDungeon.actionManager.phase.equals(GameActionManager.Phase.WAITING_ON_USER)
                        && AbstractDungeon.actionManager.cardQueue.isEmpty()
                        && AbstractDungeon.actionManager.actions.isEmpty()) {
                    return true;
                }

            // Out of combat, we want to wait one update cycle, as some screen transitions trigger further updates.
            } else {
                waitOneUpdate = true;
                previousScreenUp = newScreenUp;
                previousScreen = newScreen;
                previousPhase = newPhase;
                return false;
            }
        } else if (waitOneUpdate) {
            waitOneUpdate = false;
            return true;
        }
        // We are assuming that commands are only being submitted through our interface. Some actions that require
        // our attention, like retaining a card, occur after the end turn is queued, but the previous cases
        // cover those actions. We would like to avoid registering other state changes after the end turn
        // command but before the game actually ends your turn.
        if (inCombat && isEndTurnStillResolving()) {
            return false;
        }
        // If some other code registered a state change through registerStateChange(), or if we notice a state
        // change through the gold amount changing, we still need to wait until all actions are finished
        // resolving to claim a stable state and ask for a new command.
        if ((externalChange || previousGold != AbstractDungeon.player.gold)
                && AbstractDungeon.actionManager.phase.equals(GameActionManager.Phase.WAITING_ON_USER)
                && AbstractDungeon.actionManager.preTurnActions.isEmpty()
                && AbstractDungeon.actionManager.actions.isEmpty()
                && AbstractDungeon.actionManager.cardQueue.isEmpty()) {
            return true;
        }
        // In a grid select screen, if a confirm screen comes up or goes away, it doesn't change any other state.
        if (newScreen == AbstractDungeon.CurrentScreen.GRID) {
            boolean newGridSelectConfirmUp = AbstractDungeon.gridSelectScreen.confirmScreenUp;
            if (previousScreen == AbstractDungeon.CurrentScreen.GRID && newGridSelectConfirmUp != previousGridSelectConfirmUp) {
                return true;
            }
        }
        // Sometimes, we need to register an external change in combat while an action is resolving which brings
        // the screen up. Because the screen did not change, this is not covered by other cases.
        if (externalChange && inCombat && newScreenUp) {
            return true;
        }
        if (timeout > 0) {
            timeout -= 1;
            if(timeout == 0) {
                return true;
            }
        }
        return false;
    }

    /**
     * Detects whether the state of the game menu has changed. Right now, this only occurs when you first enter the
     * menu, either after starting Slay the Spire for the first time, or after ending a game and returning to the menu.
     *
     * @return Whether the main menu has just been entered.
     */
    public static boolean checkForMenuStateChange() {
        boolean stateChange = false;
        if (!hasPresentedOutOfGameState && CardCrawlGame.mode == CardCrawlGame.GameMode.CHAR_SELECT && CardCrawlGame.mainMenuScreen != null) {
            stateChange = true;
            hasPresentedOutOfGameState = true;
        }
        if (stateChange) {
            externalChange = false;
            waitingForCommand = true;
            boundaryKind = "terminal";
        }
        return stateChange;
    }

    /**
     * Detects a state change in AbstractDungeon, and updates all of the local variables used to detect
     * changes in the dungeon state. Sets waitingForCommand = true if a state change was registered since
     * the last command was sent.
     *
     * @return Whether a dungeon state change was detected
     */
    public static boolean checkForDungeonStateChange() {
        boolean stateChange = false;
        if (CommandExecutor.isInDungeon()) {
            stateChange = hasDungeonStateChanged();
            if (stateChange) {
                externalChange = false;
                waitingForCommand = true;
                boundaryKind = classifyDungeonBoundary();
                previousPhase = AbstractDungeon.getCurrRoom().phase;
                previousScreen = AbstractDungeon.screen;
                previousScreenUp = AbstractDungeon.isScreenUp;
                previousGold = AbstractDungeon.player.gold;
                previousGridSelectConfirmUp = AbstractDungeon.gridSelectScreen.confirmScreenUp;
                timeout = 0;
            }
        } else {
            myTurn = false;
        }
        return stateChange;
    }

    private static boolean actionManagerIsQuiescent() {
        if (!CommandExecutor.isInDungeon() || AbstractDungeon.actionManager == null) {
            return true;
        }
        GameActionManager manager = AbstractDungeon.actionManager;
        return manager.phase == GameActionManager.Phase.WAITING_ON_USER
                && manager.currentAction == null
                && manager.preTurnActions.isEmpty()
                && manager.actions.isEmpty()
                && manager.cardQueue.isEmpty();
    }

    private static String classifyDungeonBoundary() {
        if (!CommandExecutor.isInDungeon()
                || AbstractDungeon.screen == AbstractDungeon.CurrentScreen.DEATH) {
            return "terminal";
        }
        return actionManagerIsQuiescent() ? "quiescent" : "interaction_ready";
    }

    public static boolean isWaitingForCommand() {
        return waitingForCommand;
    }

    public static int getBoundarySchema() {
        return BOUNDARY_SCHEMA;
    }

    /** Whether the current state is an authoritative completion for one command. */
    public static boolean hasCompletingBoundary() {
        String kind = getBoundaryKind();
        return kind.equals("poll")
                || kind.equals("interaction_ready")
                || kind.equals("quiescent")
                || kind.equals("terminal");
    }

    public static String getBoundaryKind() {
        return pollPending ? "poll" : boundaryKind;
    }

    /** Returns the boundary for one response and consumes an explicit poll marker. */
    public static String consumeBoundaryKind() {
        String result = getBoundaryKind();
        pollPending = false;
        return result;
    }

    public static long getGameUpdateSeq() {
        return gameUpdateSeq;
    }

    public static long getDungeonUpdateSeq() {
        return dungeonUpdateSeq;
    }

    public static String getCurrentActionName() {
        if (!CommandExecutor.isInDungeon() || AbstractDungeon.actionManager == null) {
            return null;
        }
        AbstractGameAction action = AbstractDungeon.actionManager.currentAction;
        observeCurrentAction(action);
        return action == null ? null : action.getClass().getSimpleName();
    }

    public static Long getCurrentActionInstance() {
        return getCurrentActionName() == null ? null : currentActionInstance;
    }

    public static Long getCurrentActionUpdateCount() {
        return getCurrentActionName() == null ? null : currentActionUpdateCount;
    }

    public static int getActionQueueSize() {
        return CommandExecutor.isInDungeon() && AbstractDungeon.actionManager != null
                ? AbstractDungeon.actionManager.actions.size() : 0;
    }

    public static int getCardQueueSize() {
        return CommandExecutor.isInDungeon() && AbstractDungeon.actionManager != null
                ? AbstractDungeon.actionManager.cardQueue.size() : 0;
    }

    public static int getPreTurnActionQueueSize() {
        return CommandExecutor.isInDungeon() && AbstractDungeon.actionManager != null
                ? AbstractDungeon.actionManager.preTurnActions.size() : 0;
    }

    /**
     * Whether an end turn is still resolving. A boundary published while this
     * is true is mid-turn, even when the action queue looks empty, so traces
     * can be audited for the transient-empty-queue window.
     */
    public static boolean isEndTurnQueued() {
        if (!isEndTurnStillResolving()) {
            clearStaleEndTurnQueue();
            return false;
        }
        return true;
    }

    /**
     * True only while an END is still resolving inside an active combat. After
     * SuperFastMode races the end-turn into COMBAT_REWARD / battle-over, vanilla
     * can leave {@code player.endTurnQueued} stuck true. Schema 2 must not treat
     * that leftover as a mid-turn window, or the accepted END never completes.
     */
    public static boolean isEndTurnStillResolving() {
        if (!CommandExecutor.isInDungeon() || AbstractDungeon.player == null) {
            return false;
        }
        if (!AbstractDungeon.player.endTurnQueued) {
            return false;
        }
        return !combatHasEndedThroughEndTurn();
    }

    /**
     * Clears a leftover {@code endTurnQueued} once combat is observably over so
     * the next combat does not inherit a schema-2 block.
     */
    public static void clearStaleEndTurnQueue() {
        if (!CommandExecutor.isInDungeon() || AbstractDungeon.player == null) {
            return;
        }
        if (AbstractDungeon.player.endTurnQueued && combatHasEndedThroughEndTurn()) {
            AbstractDungeon.player.endTurnQueued = false;
            AbstractDungeon.player.isEndingTurn = false;
        }
    }

    private static boolean combatHasEndedThroughEndTurn() {
        if (!CommandExecutor.isInDungeon()) {
            return true;
        }
        AbstractRoom room = AbstractDungeon.getCurrRoom();
        if (room == null || room.isBattleOver || room.phase != AbstractRoom.RoomPhase.COMBAT) {
            return true;
        }
        AbstractDungeon.CurrentScreen screen = AbstractDungeon.screen;
        return screen == AbstractDungeon.CurrentScreen.COMBAT_REWARD
                || screen == AbstractDungeon.CurrentScreen.DEATH
                || screen == AbstractDungeon.CurrentScreen.VICTORY
                || screen == AbstractDungeon.CurrentScreen.UNLOCK
                || screen == AbstractDungeon.CurrentScreen.NEOW_UNLOCK;
    }
}
