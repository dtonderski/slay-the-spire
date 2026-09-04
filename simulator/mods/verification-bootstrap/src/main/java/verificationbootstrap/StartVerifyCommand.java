package verificationbootstrap;

final class StartVerifyCommand {
    static final String NAME = "start_verify";
    static final int MAX_STARTING_HP = 1_000_000;

    final String character;
    final String ascension;
    final String seed;
    final int startingHp;

    private StartVerifyCommand(
        String character,
        String ascension,
        String seed,
        int startingHp
    ) {
        this.character = character;
        this.ascension = ascension;
        this.seed = seed;
        this.startingHp = startingHp;
    }

    static boolean matches(String command) {
        if (command == null) {
            return false;
        }
        String trimmed = command.trim();
        if (trimmed.isEmpty()) {
            return false;
        }
        String[] parts = trimmed.split("\\s+");
        return parts.length > 0 && NAME.equalsIgnoreCase(parts[0]);
    }

    static StartVerifyCommand parse(String command) {
        String[] parts = command.trim().split("\\s+");
        if (parts.length != 5) {
            throw new IllegalArgumentException(
                "Expected START_VERIFY <character> <ascension> <seed> <starting-hp>"
            );
        }

        final int startingHp;
        try {
            startingHp = Integer.parseInt(parts[4]);
        } catch (NumberFormatException error) {
            throw new IllegalArgumentException("Starting HP must be an integer", error);
        }

        if (startingHp < 1 || startingHp > MAX_STARTING_HP) {
            throw new IllegalArgumentException(
                "Starting HP must be between 1 and " + MAX_STARTING_HP
            );
        }

        return new StartVerifyCommand(parts[1], parts[2], parts[3], startingHp);
    }

    String normalStartCommand() {
        return "START " + character + " " + ascension + " " + seed;
    }
}
