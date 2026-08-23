package communicationmod.patches;

import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePrefixPatch;
import com.evacipated.cardcrawl.modthespire.lib.SpireReturn;
import com.megacrit.cardcrawl.cards.AbstractCard;
import com.megacrit.cardcrawl.core.CardCrawlGame;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.rooms.ShopRoom;
import com.megacrit.cardcrawl.vfx.UpgradeShineEffect;
import com.megacrit.cardcrawl.vfx.cardManip.ShowCardBrieflyEffect;
import com.megacrit.cardcrawl.core.Settings;

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
 * Shop-purge GRID CONFIRM does not resume here. While the shop screen is up,
 * {@code ShopScreen.updatePurge} consumes {@code selectedCards} first.
 */
@SpirePatch(clz = ShopRoom.class, method = "updatePurge")
public class ShopRoomPurgePatch {

    @SpirePrefixPatch
    public static SpireReturn<Void> Prefix(ShopRoom __instance) {
        if (AbstractDungeon.gridSelectScreen.selectedCards.isEmpty()) {
            return SpireReturn.Continue();
        }
        if (AbstractDungeon.gridSelectScreen.forPurge) {
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
}
