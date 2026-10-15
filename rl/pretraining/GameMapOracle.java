import com.badlogic.gdx.*;
import com.badlogic.gdx.backends.lwjgl.LwjglFiles;
import com.badlogic.gdx.graphics.*;
import com.badlogic.gdx.graphics.g2d.BitmapFont;
import com.badlogic.gdx.math.MathUtils;
import com.badlogic.gdx.utils.GdxNativesLoader;
import com.google.gson.*;
import com.megacrit.cardcrawl.core.*;
import com.megacrit.cardcrawl.dungeons.*;
import com.megacrit.cardcrawl.helpers.*;
import com.megacrit.cardcrawl.localization.LocalizedStrings;
import com.megacrit.cardcrawl.map.*;
import com.megacrit.cardcrawl.random.Random;
import com.megacrit.cardcrawl.unlock.UnlockTracker;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import sun.misc.Unsafe;

/** Original game generateMap bytecode with graphics-only headless scaffolding. */
public final class GameMapOracle {
    static void headless() throws Exception {
        GdxNativesLoader.load();
        InvocationHandler rendering = (proxy, method, values) -> {
            switch (method.getName()) {
                case "getDisplayMode": return new Graphics.DisplayMode(1920, 1080, 60, 32) {};
                case "getDisplayModes": return new Graphics.DisplayMode[]{new Graphics.DisplayMode(1920, 1080, 60, 32) {}};
                case "getWidth": return 1920;
                case "getHeight": return 1080;
            }
            Class<?> type = method.getReturnType();
            if (type == boolean.class) return false;
            if (type == int.class) return 0;
            if (type == long.class) return 0L;
            if (type == float.class) return 0.0f;
            if (type == double.class) return 0.0;
            return null;
        };
        Gdx.gl = Gdx.gl20 = (GL20) Proxy.newProxyInstance(GameMapOracle.class.getClassLoader(), new Class<?>[]{GL20.class}, rendering);
        Gdx.graphics = (Graphics) Proxy.newProxyInstance(GameMapOracle.class.getClassLoader(), new Class<?>[]{Graphics.class}, rendering);
        Gdx.files = new LwjglFiles();
        Settings.scale = Settings.xScale = Settings.yScale = 1.0f;
        Settings.language = Settings.GameLanguage.ENG;
        CardCrawlGame.languagePack = new LocalizedStrings();
        BitmapFont font = new BitmapFont();
        for (Field field : FontHelper.class.getDeclaredFields()) {
            if (field.getType() == BitmapFont.class && Modifier.isStatic(field.getModifiers())) {
                field.setAccessible(true); field.set(null, font);
            }
        }
        for (Class<?> owner : new Class<?>[]{Settings.class, UnlockTracker.class}) {
            for (Field field : owner.getDeclaredFields()) {
                if (field.getType() == Prefs.class && Modifier.isStatic(field.getModifiers())) {
                    field.setAccessible(true);
                    Prefs prefs = new Prefs(); prefs.data = new HashMap<>(); field.set(null, prefs);
                }
            }
        }
        ImageMaster.UI_GOLD = new Texture(1, 1, Pixmap.Format.RGBA8888);
        // Explicit normal-run scope, not a hidden dependency on local unlocks.
        Settings.isEndless = false;
        Settings.isFinalActAvailable = false;
        Settings.hasEmeraldKey = false;
        ModHelper.setModsFalse();
    }

    public static void main(String[] args) throws Exception {
        PrintStream output = System.out;
        System.setOut(System.err);
        long cosmeticSeed = args.length == 0 ? 1776L : Long.parseLong(args[0]);
        MathUtils.random.setSeed(cosmeticSeed);
        headless();
        if (!CardCrawlGame.VERSION_NUM.equals("[V2.3.4] (12-18-2022)")) {
            throw new IllegalArgumentException("Harness initialization audited only for PC 12-18-2022; review other builds first");
        }
        System.err.println("Authority: " + CardCrawlGame.VERSION_NUM + " " + AbstractDungeon.class.getProtectionDomain().getCodeSource().getLocation());
        Field unsafeField = Unsafe.class.getDeclaredField("theUnsafe");
        unsafeField.setAccessible(true);
        Unsafe allocator = (Unsafe) unsafeField.get(null);
        Method generate = AbstractDungeon.class.getDeclaredMethod("generateMap");
        generate.setAccessible(true);
        BufferedReader input = new BufferedReader(new InputStreamReader(System.in));
        Gson gson = new Gson();
        String line;
        while ((line = input.readLine()) != null) {
            JsonObject request = new JsonParser().parse(line).getAsJsonObject();
            if (request.entrySet().size() != 3 || !request.has("seed") || !request.has("act") || !request.has("ascension")) throw new IllegalArgumentException("seed, act, ascension only");
            long seed = Long.parseLong(request.get("seed").getAsString());
            int act = Integer.parseInt(request.get("act").getAsString());
            int ascension = Integer.parseInt(request.get("ascension").getAsString());
            if (act < 1 || act > 3 || ascension != 0) throw new IllegalArgumentException("normal A0 acts 1..3 only");
            Class<?> dungeon = act == 1 ? Exordium.class : act == 2 ? TheCity.class : TheBeyond.class;
            // This method assigns static initial probabilities only; no dungeon
            // constructor, saved state, recorded route or observation is used.
            Method chances = dungeon.getDeclaredMethod("initializeLevelSpecificChances");
            chances.setAccessible(true);
            Settings.seed = seed;
            AbstractDungeon.actNum = act;
            AbstractDungeon.ascensionLevel = ascension;
            chances.invoke(allocator.allocateInstance(dungeon));
            long offset = act == 1 ? act : act == 2 ? act * 100L : act * 200L;
            AbstractDungeon.mapRng = new Random(Math.addExact(seed, offset));
            // Separate cosmetic draws cannot consume the map stream.
            MathUtils.random.setSeed(cosmeticSeed);
            AbstractDungeon.fadeColor = new Color(0, 0, 0, 1);
            generate.invoke(null);
            JsonObject result = new JsonObject();
            result.addProperty("seed", seed);
            result.addProperty("act", act);
            result.addProperty("ascension", ascension);
            JsonArray nodes = new JsonArray();
            JsonArray starts = new JsonArray();
            for (ArrayList<MapRoomNode> row : AbstractDungeon.map) for (MapRoomNode node : row) {
                if (!node.hasEdges()) continue;
                if (node.y == 0) starts.add(new JsonPrimitive(node.x));
                JsonObject item = new JsonObject();
                item.addProperty("row", node.y);
                item.addProperty("x", node.x);
                item.addProperty("symbol", node.getRoomSymbol(true));
                JsonArray edges = new JsonArray();
                for (MapEdge edge : node.getEdges()) {
                    JsonArray destination = new JsonArray();
                    destination.add(new JsonPrimitive(edge.dstY));
                    destination.add(new JsonPrimitive(edge.dstX));
                    edges.add(destination);
                }
                item.add("children", edges);
                nodes.add(item);
            }
            result.add("starts", starts);
            result.add("nodes", nodes);
            output.println(gson.toJson(result));
        }
    }
}
