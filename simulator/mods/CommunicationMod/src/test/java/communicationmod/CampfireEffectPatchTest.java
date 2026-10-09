package communicationmod;

import com.evacipated.cardcrawl.modthespire.lib.SpireInsertLocator;
import com.evacipated.cardcrawl.modthespire.lib.SpirePatch;
import com.megacrit.cardcrawl.vfx.campfire.CampfireDigEffect;
import com.megacrit.cardcrawl.vfx.campfire.CampfireRecallEffect;
import com.megacrit.cardcrawl.vfx.FastCardObtainEffect;
import communicationmod.patches.FastCardObtainEffectDiagnosticPatch;
import javassist.ClassPool;
import javassist.CtBehavior;
import communicationmod.patches.CampfireDigEffectPatch;
import communicationmod.patches.CampfireRecallEffectPatch;
import communicationmod.patches.GremlinMatchGamePatch;
import org.junit.Test;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

public class CampfireEffectPatchTest {
    @Test
    public void digEffectPatchIsRegisteredOnTargetUpdatePath() {
        SpirePatch patch = CampfireDigEffectPatch.class.getAnnotation(SpirePatch.class);
        assertNotNull(patch);
        assertEquals(CampfireDigEffect.class, patch.clz());
        assertEquals("update", patch.method());
    }

    @Test
    public void fastCardObtainDiagnosticPatchTargetsUpdate() {
        SpirePatch patch = FastCardObtainEffectDiagnosticPatch.class.getAnnotation(SpirePatch.class);
        assertNotNull(patch);
        assertEquals(FastCardObtainEffect.class, patch.clz());
        assertEquals("update", patch.method());
    }

    @Test
    public void targetJavassistLocatorsResolveForDigRecallAndMatch() throws Exception {
        assertPatchLocators(CampfireDigEffectPatch.class, CampfireDigEffect.class, "update");
        assertPatchLocators(CampfireRecallEffectPatch.class, CampfireRecallEffect.class, "update");
        assertPatchLocators(GremlinMatchGamePatch.class,
                com.megacrit.cardcrawl.events.shrines.GremlinMatchGame.class,
                "updateMatchGameLogic");
    }

    private static void assertPatchLocators(Class<?> patch, Class<?> target, String method)
            throws Exception {
        ClassPool pool = ClassPool.getDefault();
        CtBehavior behavior = pool.get(target.getName()).getDeclaredMethod(method);
        int[] locatorCount = new int[]{0};
        resolveLocators(patch, behavior, locatorCount);
        assertTrue("No Javassist locators found for " + patch.getName(), locatorCount[0] > 0);
    }

    private static void resolveLocators(Class<?> type, CtBehavior behavior, int[] locatorCount)
            throws Exception {
        for (Class<?> nested : type.getDeclaredClasses()) {
            if (SpireInsertLocator.class.isAssignableFrom(nested)) {
                java.lang.reflect.Constructor<?> constructor = nested.getDeclaredConstructor();
                constructor.setAccessible(true);
                SpireInsertLocator locator = (SpireInsertLocator) constructor.newInstance();
                int[] locations = locator.Locate(behavior);
                assertNotNull(locations);
                assertTrue(nested.getName(), locations.length > 0);
                locatorCount[0] += 1;
            }
            resolveLocators(nested, behavior, locatorCount);
        }
    }
}
