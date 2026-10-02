package communicationmod;

import org.junit.Test;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

public class ShopPotionChoiceTest {
    @Test
    public void potionChoiceRequiresAnEmptySlotAndNoSozu() {
        assertTrue(ChoiceScreenUtils.canPurchaseShopPotion(false, true));
        assertFalse(ChoiceScreenUtils.canPurchaseShopPotion(true, true));
        assertFalse(ChoiceScreenUtils.canPurchaseShopPotion(false, false));
        assertFalse(ChoiceScreenUtils.canPurchaseShopPotion(true, false));
    }

    @Test
    public void fullBeltHidesPotionRewardsButKeepsOtherRewards() {
        assertTrue(ChoiceScreenUtils.canClaimCombatReward(false, false));
        assertTrue(ChoiceScreenUtils.canClaimCombatReward(false, true));
        assertTrue(ChoiceScreenUtils.canClaimCombatReward(true, true));
        assertFalse(ChoiceScreenUtils.canClaimCombatReward(true, false));
    }
}
