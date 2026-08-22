package communicationmod.patches;

import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePostfixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePrefixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpireReturn;
import com.megacrit.cardcrawl.cards.AbstractCard;
import com.megacrit.cardcrawl.core.CardCrawlGame;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.rooms.ShopRoom;
import com.megacrit.cardcrawl.vfx.UpgradeShineEffect;
import com.megacrit.cardcrawl.vfx.cardManip.ShowCardBrieflyEffect;
import com.megacrit.cardcrawl.core.Settings;
import communicationmod.GameStateListener;

/**
 * Vanilla {@code ShopRoom.updatePurge()} purges every non-empty
 * {@code gridSelectScreen.selectedCards} list without checking {@code forPurge}.
 *
 * When {@code CampfireSmithEffect} opens the upgrade grid and completes in the same
 * large-delta frame (room already COMPLETE while GRID is still up), a later CONFIRM
 * leaves the smithed card in {@code selectedCards} with {@code forUpgrade=true}.
 * Entering a shop then accidentally charges {@code actualPurgeCost} and removes the card.
 *
 * Guard: only run the purge path for genuine purge selections. If an upgrade selection
 * is still pending, apply the smith upgrade instead (same outcome as CampfireSmithEffect).
 *
 * CommunicationMod shop-purge GRID CONFIRM also blocks {@link GameStateListener}
 * until this method processes a purge selection. {@code ShopScreen.purgeCard}
 * only opens the grid; resuming there never unblocks CONFIRM and would resume
 * too early when the purge option is chosen.
 */
@SpirePatch(clz = ShopRoom.class, method = "updatePurge")
public class ShopRoomPurgePatch {

    private static boolean resumeAfterThisUpdate = false;

    /**
     * {@code updatePurge} runs every shop frame. Resume only when a genuine
     * purge selection is present so CHOOSE purge (opening the grid) does not
     * ack before CONFIRM, and empty frames after CONFIRM stay blocked until
     * the grid has filled {@code selectedCards}.
     */
    public static boolean shouldResumeAfterShopUpdatePurge(
            boolean hadSelectedCards,
            boolean forPurge
    ) {
        return hadSelectedCards && forPurge;
    }

    @SpirePrefixPatch
    public static SpireReturn<Void> Prefix(ShopRoom __instance) {
        boolean hadSelectedCards = !AbstractDungeon.gridSelectScreen.selectedCards.isEmpty();
        boolean forPurge = AbstractDungeon.gridSelectScreen.forPurge;
        resumeAfterThisUpdate = shouldResumeAfterShopUpdatePurge(hadSelectedCards, forPurge);
        if (!hadSelectedCards) {
            return SpireReturn.Continue();
        }
        if (forPurge) {
            return SpireReturn.Continue();
        }
        if (AbstractDungeon.gridSelectScreen.forUpgrade) {
            for (AbstractCard card : AbstractDungeon.gridSelectScreen.selectedCards) {
                AbstractDungeon.effectsQueue.add(
                        new UpgradeShineEffect(Settings.WIDTH / 2.0f, Settings.HEIGHT / 2.0f));
                CardCrawlGame.metricData.campfire_upgraded++;
                CardCrawlGame.metricData.addCampfireChoiceData("SMITH", card.getMetricID());
                card.upgrade();
                AbstractDungeon.player.bottledCardUpgradeCheck(card);
                AbstractDungeon.effectsQueue.add(
                        new ShowCardBrieflyEffect(card.makeStatEquivalentCopy()));
            }
            AbstractDungeon.gridSelectScreen.selectedCards.clear();
            return SpireReturn.Return(null);
        }
        // Non-purge, non-upgrade leftover selection must not spend gold.
        AbstractDungeon.gridSelectScreen.selectedCards.clear();
        return SpireReturn.Return(null);
    }

    @SpirePostfixPatch
    public static void Postfix(ShopRoom __instance) {
        if (resumeAfterThisUpdate) {
            resumeAfterThisUpdate = false;
            GameStateListener.resumeStateUpdate();
        }
    }

    public static void resumeListenerIfShopPurgeCompleted(
            boolean hadSelectedCards,
            boolean forPurge
    ) {
        if (shouldResumeAfterShopUpdatePurge(hadSelectedCards, forPurge)) {
            GameStateListener.resumeStateUpdate();
        }
    }
}
