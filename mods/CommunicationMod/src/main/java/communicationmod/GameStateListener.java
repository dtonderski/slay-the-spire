package communicationmod;

import com.megacrit.cardcrawl.actions.AbstractGameAction;
import com.megacrit.cardcrawl.actions.GameActionManager;
import com.megacrit.cardcrawl.core.CardCrawlGame;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.monsters.AbstractMonster;
import com.megacrit.cardcrawl.neow.NeowRoom;
import com.megacrit.cardcrawl.rooms.AbstractRoom;
import com.megacrit.cardcrawl.rooms.EventRoom;
import com.megacrit.cardcrawl.rooms.VictoryRoom;
import com.megacrit.cardcrawl.vfx.AbstractGameEffect;
import com.megacrit.cardcrawl.vfx.ObtainKeyEffect;
import com.megacrit.cardcrawl.vfx.cardManip.ShowCardAndObtainEffect;

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
    //    payload carries end_turn_queued.
    // 3: quiescent combat readiness waits for every active monster's target
    //    lifecycle to initialize its public intent.
    // 4: a deferred out-of-combat stabilization update cannot survive into
    //    combat and complete a command before its queued card resolves.
    // 5: every gameplay boundary carries a monotonic command-execution fence,
    //    so a late state from the preceding command cannot complete the next.
    // 6: readiness also waits for gameplay-mutating dungeon effects.
    //    ObtainKeyEffect and ShowCardAndObtainEffect mutate gameplay state
    //    after action queues are otherwise quiescent.
    // 7: every command attempt has a target-visible identity. Non-STATE/PROFILE
    //    attempts, including rejections, advance command_execution_seq once.
    //    command_settlement_seq advances once when a gameplay command reaches a
    //    published interaction-ready/quiescent/terminal boundary. Completions
    //    echo command_response_id and command_response_kind.
    private static final int BOUNDARY_SCHEMA = 7;
    private static String boundaryKind = "unknown";
    private static boolean pollPending = false;
    private static long gameUpdateSeq = 0L;
    private static long dungeonUpdateSeq = 0L;
    // Process-lifetime monotonic counters. Do not reset them between runs: the
    // external bridge also persists, and uses these values as command fences.
    private static long commandExecutionSeq = 0L;
    private static long commandSettlementSeq = 0L;
    private static String activeCommandId = null;
    private static String commandResponseId = null;
    private static String commandResponseKind = "unsolicited";
    private static boolean transactionPending = false;
    private static boolean waitingBeforeCommand = false;
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
        beforeCommand(null, "play");
    }

    /**
     * Begins a parsed command attempt before target execution. This makes the
     * command identity and attempt sequence own every synchronous mutation too.
     */
    public static void beforeCommand(String commandId, String verb) {
        if (CommandEnvelope.isObservationVerb(verb)) {
            if ("state".equals(verb)) {
                pollPending = true;
                activeCommandId = commandId;
            }
            return;
        }
        waitingBeforeCommand = waitingForCommand;
        waitingForCommand = false;
        boundaryKind = "unknown";
        pollPending = false;
        commandExecutionSeq += 1L;
        activeCommandId = commandId;
        transactionPending = true;
        commandResponseKind = "unsolicited";
        commandResponseId = null;
    }

    /** Completes bookkeeping after a successfully executed command. */
    public static void afterCommand(String commandId, String verb, boolean stateChanged) {
        // The attempt must be registered before CommandExecutor runs. Gameplay
        // settlement is stamped later by the authoritative state boundary.
    }

    /** Records rejection of the command attempt begun by beforeCommand. */
    public static void afterRejectedCommand(String commandId, String verb) {
        if (!CommandEnvelope.isObservationVerb(verb)) {
            waitingForCommand = waitingBeforeCommand;
            waitingBeforeCommand = false;
            transactionPending = false;
        }
        boundaryKind = "unknown";
        pollPending = false;
        commandResponseId = commandId;
        commandResponseKind = "rejected";
        activeCommandId = null;
    }

    /** Clears any stale identity when command framing itself could not be parsed. */
    public static void afterUnidentifiedRejectedCommand() {
        commandResponseId = null;
        commandResponseKind = "rejected";
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

    public static boolean isStateUpdateBlocked() {
        return blocked;
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
        waitOneUpdate = retainDeferredOutOfCombatUpdate(waitOneUpdate, inCombat);
        // Lots of stuff can happen while the dungeon is fading out, but nothing that requires input from the user.
        if (AbstractDungeon.isFadingOut || AbstractDungeon.isFadingIn) {
            return false;
        }
        // Several effects that look visual own delayed gameplay mutations.
        // ObtainKeyEffect grants keys only when its duration expires, and
        // ShowCardAndObtainEffect commits cards to the master deck. Decorative
        // effects may coexist with input; these two must finish first.
        if (!dungeonEffectQueuesAreSettled()) {
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
                else if (quiescentCombatBoundaryIsReady()) {
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
        if (inCombat && AbstractDungeon.player.endTurnQueued) {
            return false;
        }
        // If some other code registered a state change through registerStateChange(), or if we notice a state
        // change through the gold amount changing, we still need to wait until all actions are finished
        // resolving to claim a stable state and ask for a new command.
        if ((externalChange || previousGold != AbstractDungeon.player.gold)
                && (!inCombat || quiescentCombatBoundaryIsReady())
                && AbstractDungeon.actionManager.phase.equals(GameActionManager.Phase.WAITING_ON_USER)
                && AbstractDungeon.actionManager.currentAction == null
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
            stampPublishedGameplayResponse();
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
                stampPublishedGameplayResponse();
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

    private static boolean monsterIntentsAreInitialized() {
        for (AbstractMonster monster : AbstractDungeon.getMonsters().monsters) {
            if (!monster.isDeadOrEscaped()
                    && monster.intent == AbstractMonster.Intent.DEBUG) {
                return false;
            }
        }
        return true;
    }

    static boolean retainDeferredOutOfCombatUpdate(boolean pending, boolean inCombat) {
        return pending && !inCombat;
    }

    static boolean effectQueuesAreSettled(
            int effects,
            int topLevelEffects,
            int queuedTopLevelEffects
    ) {
        return effects == 0 && topLevelEffects == 0 && queuedTopLevelEffects == 0;
    }

    private static boolean isPendingGameplayEffect(AbstractGameEffect effect) {
        return !effect.isDone
                && (effect instanceof ObtainKeyEffect
                || effect instanceof ShowCardAndObtainEffect);
    }

    private static int pendingGameplayEffectCount(Iterable<AbstractGameEffect> effects) {
        int count = 0;
        for (AbstractGameEffect effect : effects) {
            if (isPendingGameplayEffect(effect)) {
                count += 1;
            }
        }
        return count;
    }

    private static boolean dungeonEffectQueuesAreSettled() {
        return effectQueuesAreSettled(
                pendingGameplayEffectCount(AbstractDungeon.effectList),
                pendingGameplayEffectCount(AbstractDungeon.topLevelEffects),
                pendingGameplayEffectCount(AbstractDungeon.topLevelEffectsQueue)
        );
    }

    static boolean isQuiescentCombatBoundaryReady(
            boolean endTurnQueued,
            boolean actionManagerQuiescent,
            boolean monsterIntentsInitialized
    ) {
        return !endTurnQueued && actionManagerQuiescent && monsterIntentsInitialized;
    }

    private static boolean quiescentCombatBoundaryIsReady() {
        return isQuiescentCombatBoundaryReady(
                AbstractDungeon.player.endTurnQueued,
                actionManagerIsQuiescent(),
                monsterIntentsAreInitialized()
        );
    }

    private static String classifyDungeonBoundary() {
        if (!CommandExecutor.isInDungeon()
                || AbstractDungeon.screen == AbstractDungeon.CurrentScreen.DEATH) {
            return "terminal";
        }
        return actionManagerIsQuiescent() ? "quiescent" : "interaction_ready";
    }

    static void stampPublishedGameplayResponse() {
        if (!transactionPending) {
            commandResponseKind = "unsolicited";
            commandResponseId = null;
            return;
        }
        commandSettlementSeq += 1L;
        commandResponseKind = "settled";
        commandResponseId = activeCommandId;
        transactionPending = false;
        waitingBeforeCommand = false;
        activeCommandId = null;
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
        if (pollPending) {
            commandResponseKind = "poll";
            commandResponseId = activeCommandId;
            activeCommandId = null;
            pollPending = false;
            return "poll";
        }
        return boundaryKind;
    }

    public static long getCommandSettlementSeq() {
        return commandSettlementSeq;
    }

    public static String getCommandResponseId() {
        return commandResponseId;
    }

    public static String getCommandResponseKind() {
        return commandResponseKind;
    }

    public static boolean isTransactionPending() {
        return transactionPending;
    }

    public static long getGameUpdateSeq() {
        return gameUpdateSeq;
    }

    public static long getDungeonUpdateSeq() {
        return dungeonUpdateSeq;
    }

    public static long getCommandExecutionSeq() {
        return commandExecutionSeq;
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

    public static int getEffectQueueSize() {
        return CommandExecutor.isInDungeon()
                ? pendingGameplayEffectCount(AbstractDungeon.effectList) : 0;
    }

    public static int getTopLevelEffectQueueSize() {
        return CommandExecutor.isInDungeon()
                ? pendingGameplayEffectCount(AbstractDungeon.topLevelEffects) : 0;
    }

    public static int getQueuedTopLevelEffectQueueSize() {
        return CommandExecutor.isInDungeon()
                ? pendingGameplayEffectCount(AbstractDungeon.topLevelEffectsQueue) : 0;
    }

    /**
     * Whether an end turn is still resolving. A boundary published while this
     * is true is mid-turn, even when the action queue looks empty, so traces
     * can be audited for the transient-empty-queue window.
     */
    static boolean isEndTurnUnresolved(
            boolean inDungeon,
            boolean hasPlayer,
            AbstractRoom.RoomPhase roomPhase,
            boolean endTurnQueued
    ) {
        return inDungeon
                && hasPlayer
                && roomPhase == AbstractRoom.RoomPhase.COMBAT
                && endTurnQueued;
    }

    public static boolean isEndTurnQueued() {
        boolean inDungeon = CommandExecutor.isInDungeon();
        AbstractRoom currentRoom = inDungeon ? AbstractDungeon.getCurrRoom() : null;
        return isEndTurnUnresolved(
                inDungeon,
                AbstractDungeon.player != null,
                currentRoom == null ? null : currentRoom.phase,
                AbstractDungeon.player != null && AbstractDungeon.player.endTurnQueued
        );
    }
}
