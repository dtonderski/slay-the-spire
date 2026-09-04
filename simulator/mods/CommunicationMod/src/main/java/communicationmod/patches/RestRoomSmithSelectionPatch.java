package communicationmod.patches;

import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePostfixPatch;
import com.megacrit.cardcrawl.cards.AbstractCard;
import com.megacrit.cardcrawl.core.CardCrawlGame;
import com.megacrit.cardcrawl.core.Settings;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.rooms.RestRoom;
import com.megacrit.cardcrawl.vfx.UpgradeShineEffect;
import com.megacrit.cardcrawl.vfx.cardManip.ShowCardBrieflyEffect;

/**
 * If {@code CampfireSmithEffect} already finished (e.g. large delta completed it in the
 * same frame that opened the upgrade grid), a later CONFIRM leaves the card in
 * {@code selectedCards} with {@code forUpgrade=true} and no live effect to apply it.
 *
 * Drain that selection from {@code RestRoom.update} so the upgrade still lands before
 * the player can leave for a shop (where vanilla {@code updatePurge} would otherwise
 * spend gold and delete the card).
 */
@SpirePatch(clz = RestRoom.class, method = "update")
public class RestRoomSmithSelectionPatch {

    @SpirePostfixPatch
    public static void Postfix(RestRoom __instance) {
        if (AbstractDungeon.isScreenUp) {
            return;
        }
        if (!AbstractDungeon.gridSelectScreen.forUpgrade) {
            return;
        }
        if (AbstractDungeon.gridSelectScreen.selectedCards.isEmpty()) {
            return;
        }
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
        __instance.fadeIn();
    }
}
