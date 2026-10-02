package communicationmod;

import org.junit.Assert;

import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Captures mutable static test seams so fixtures do not depend on test order. */
final class TestStateSnapshot {
    private static final class Value {
        private final Object reference;
        private final Object contents;

        private Value(Object reference, Object contents) {
            this.reference = reference;
            this.contents = contents;
        }
    }

    private final Map<Field, Value> values;

    private TestStateSnapshot(Map<Field, Value> values) {
        this.values = values;
    }

    static TestStateSnapshot of(Class<?>... types) {
        Map<Field, Value> values = new LinkedHashMap<>();
        try {
            for (Class<?> type : types) {
                for (Field field : type.getDeclaredFields()) {
                    int modifiers = field.getModifiers();
                    if (!Modifier.isStatic(modifiers) || Modifier.isFinal(modifiers)) {
                        continue;
                    }
                    field.setAccessible(true);
                    Object reference = field.get(null);
                    values.put(field, new Value(reference, copyContents(reference)));
                }
            }
            return new TestStateSnapshot(values);
        } catch (ReflectiveOperationException exception) {
            throw new AssertionError(exception);
        }
    }

    private static Object copyContents(Object value) {
        if (value instanceof List) {
            return new ArrayList<>((List<?>) value);
        }
        if (value instanceof Map) {
            return new LinkedHashMap<>((Map<?, ?>) value);
        }
        if (value instanceof Set) {
            return new LinkedHashSet<>((Set<?>) value);
        }
        return null;
    }

    void restore() {
        AssertionError failure = null;
        for (Map.Entry<Field, Value> entry : values.entrySet()) {
            try {
                restoreContents(entry.getValue());
                entry.getKey().set(null, entry.getValue().reference);
            } catch (ReflectiveOperationException | RuntimeException exception) {
                if (failure == null) {
                    failure = new AssertionError(exception);
                }
            }
        }
        if (failure != null) {
            throw failure;
        }
    }

    private static void restoreContents(Value value) {
        if (value.contents == null || value.reference == null) {
            return;
        }
        if (value.reference instanceof List) {
            List<Object> list = (List<Object>) value.reference;
            list.clear();
            list.addAll((List<?>) value.contents);
        } else if (value.reference instanceof Map) {
            Map<Object, Object> map = (Map<Object, Object>) value.reference;
            map.clear();
            map.putAll((Map<?, ?>) value.contents);
        } else if (value.reference instanceof Set) {
            Set<Object> set = (Set<Object>) value.reference;
            set.clear();
            set.addAll((Set<?>) value.contents);
        }
    }

    /** Verifies both static identity and the contents of in-place collections. */
    void assertRestored() {
        try {
            for (Map.Entry<Field, Value> entry : values.entrySet()) {
                Value value = entry.getValue();
                Object current = entry.getKey().get(null);
                if (entry.getKey().getType().isPrimitive()) {
                    Assert.assertEquals(entry.getKey().toString(), value.reference, current);
                } else {
                    Assert.assertSame(entry.getKey().toString(), value.reference, current);
                }
                if (value.contents != null) {
                    Assert.assertEquals(entry.getKey().toString(), value.contents, current);
                }
            }
        } catch (ReflectiveOperationException exception) {
            throw new AssertionError(exception);
        }
    }
}
