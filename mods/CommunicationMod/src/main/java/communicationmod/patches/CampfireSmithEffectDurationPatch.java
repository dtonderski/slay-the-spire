package communicationmod.patches;

import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.evacipated.cardcrawl.modthespire.lib.SpirePrefixPatch;
import com.megacrit.cardcrawl.dungeons.AbstractDungeon;
import com.megacrit.cardcrawl.vfx.campfire.CampfireSmithEffect;

/**
 * Prevent {@code CampfireSmithEffect} from completing in the same update that opens the
 * upgrade grid when a single large {@code deltaTime} drives {@code duration} from 1.5
 * through 0. That leaves {@code room.phase = COMPLETE} while GRID is still up; CONFIRM
 * then parks the card in {@code selectedCards} with no live effect to apply the upgrade,
 * and the next shop visit purges it via {@code ShopRoom.updatePurge()}.
 *
 * While the upgrade grid is up (or a confirmed upgrade selection is pending), keep
 * {@code duration} positive so the effect stays alive to process {@code selectedCards}.
 */
@SpirePatch(clz = CampfireSmithEffect.class, method = "update")
public class CampfireSmithEffectDurationPatch {

    private static final float MIN_PENDING_DURATION = 0.25f;

    @SpirePrefixPatch
    public static void Prefix(CampfireSmithEffect _instance) {
        boolean upgradeGridUp =
                AbstractDungeon.isScreenUp && AbstractDungeon.gridSelectScreen.forUpgrade;
        boolean pendingUpgradeSelection =
                AbstractDungeon.gridSelectScreen.forUpgrade
                        && !AbstractDungeon.gridSelectScreen.selectedCards.isEmpty();
        if (!upgradeGridUp && !pendingUpgradeSelection) {
            return;
        }
        if (_instance.duration < MIN_PENDING_DURATION) {
            _instance.duration = MIN_PENDING_DURATION;
        }
        // Keep the effect alive until it can process the upgrade selection.
        _instance.isDone = false;
    }
}
