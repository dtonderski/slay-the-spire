package verificationbootstrap;

public final class StartVerifyCommandTest {
    private StartVerifyCommandTest() {
    }

    public static void main(String[] args) {
        parsesCoverageStartCommand();
        matchesOnlyCoverageStartCommand();
        rejectsInvalidStartingHp();
        rejectsInvalidShape();
        System.out.println("verification-bootstrap tests passed");
    }

    private static void parsesCoverageStartCommand() {
        StartVerifyCommand parsed = StartVerifyCommand.parse(
            "  START_VERIFY IRONCLAD 0 CODEX04 1000  "
        );
        assertEquals("IRONCLAD", parsed.character);
        assertEquals("0", parsed.ascension);
        assertEquals("CODEX04", parsed.seed);
        assertEquals(1000, parsed.startingHp);
        assertEquals("START IRONCLAD 0 CODEX04", parsed.normalStartCommand());
    }

    private static void matchesOnlyCoverageStartCommand() {
        assertTrue(StartVerifyCommand.matches("start_verify ironclad 0 seed 1000"));
        assertTrue(StartVerifyCommand.matches("START_VERIFY"));
        assertFalse(StartVerifyCommand.matches("START IRONCLAD 0 SEED"));
        assertFalse(StartVerifyCommand.matches(null));
        assertFalse(StartVerifyCommand.matches("  "));
    }

    private static void rejectsInvalidStartingHp() {
        assertThrows("Starting HP must be an integer", "START_VERIFY IRONCLAD 0 SEED nope");
        assertThrows("Starting HP must be between", "START_VERIFY IRONCLAD 0 SEED 0");
        assertThrows(
            "Starting HP must be between",
            "START_VERIFY IRONCLAD 0 SEED " + (StartVerifyCommand.MAX_STARTING_HP + 1)
        );
    }

    private static void rejectsInvalidShape() {
        assertThrows("Expected START_VERIFY", "START_VERIFY IRONCLAD 0 SEED");
        assertThrows("Expected START_VERIFY", "START_VERIFY IRONCLAD 0 SEED 1000 extra");
    }

    private static void assertThrows(String expectedMessage, String command) {
        try {
            StartVerifyCommand.parse(command);
            throw new AssertionError("Expected command to be rejected: " + command);
        } catch (IllegalArgumentException error) {
            if (error.getMessage() == null || !error.getMessage().contains(expectedMessage)) {
                throw new AssertionError(
                    "Expected error containing '" + expectedMessage + "' but got '" + error.getMessage() + "'"
                );
            }
        }
    }

    private static void assertTrue(boolean value) {
        if (!value) {
            throw new AssertionError("Expected true");
        }
    }

    private static void assertFalse(boolean value) {
        if (value) {
            throw new AssertionError("Expected false");
        }
    }

    private static void assertEquals(Object expected, Object actual) {
        if (expected == null ? actual != null : !expected.equals(actual)) {
            throw new AssertionError("Expected " + expected + " but got " + actual);
        }
    }

    private static void assertEquals(int expected, int actual) {
        if (expected != actual) {
            throw new AssertionError("Expected " + expected + " but got " + actual);
        }
    }
}
