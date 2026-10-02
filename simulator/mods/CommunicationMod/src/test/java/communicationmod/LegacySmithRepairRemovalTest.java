package communicationmod;

import org.junit.Test;
import static org.junit.Assert.assertNull;

public class LegacySmithRepairRemovalTest {
    @Test
    public void producerDoesNotShipSmithStateRepairPatches() {
        for (String name : new String[]{"CampfireSmithEffectDurationPatch",
                "RestRoomSmithSelectionPatch", "ShopRoomPurgePatch"}) {
            assertNull("state-repair class must not remain in build output: " + name,
                    getClass().getResource("/communicationmod/patches/" + name + ".class"));
        }
    }
}
