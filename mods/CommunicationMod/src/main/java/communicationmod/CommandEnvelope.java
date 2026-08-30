package communicationmod;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

/**
 * Schema-7 command framing. JSON envelopes carry a target-visible command
 * identity; raw text remains valid for legacy/manual clients.
 */
public final class CommandEnvelope {
    public final String commandId;
    public final String command;

    private CommandEnvelope(String commandId, String command) {
        this.commandId = commandId;
        this.command = command;
    }

    public static CommandEnvelope parse(String raw) throws InvalidCommandException {
        if (raw == null) {
            throw new InvalidCommandException("command is missing");
        }
        String trimmed = raw.trim();
        if (trimmed.isEmpty()) {
            throw new InvalidCommandException("command is empty");
        }
        if (trimmed.charAt(0) != '{') {
            return new CommandEnvelope(null, trimmed);
        }
        JsonElement parsed;
        try {
            parsed = new JsonParser().parse(trimmed);
        } catch (RuntimeException e) {
            throw new InvalidCommandException("command envelope is not valid JSON");
        }
        if (!parsed.isJsonObject()) {
            throw new InvalidCommandException("command envelope must be a JSON object");
        }
        JsonObject object = parsed.getAsJsonObject();
        if (object.entrySet().size() != 2 || !object.has("command_id") || !object.has("command")) {
            throw new InvalidCommandException(
                    "command envelope must contain only command_id and command"
            );
        }
        String commandId = requiredNonemptyString(object, "command_id");
        if (commandId.length() > 200 || containsWhitespaceOrControl(commandId)) {
            throw new InvalidCommandException(
                    "command_id must be a nonempty command token of at most 200 characters"
            );
        }
        String command = requiredNonemptyString(object, "command");
        return new CommandEnvelope(commandId, command);
    }

    public String verb() {
        return verbOf(command);
    }

    public static String verbOf(String command) {
        String trimmed = command.trim();
        int space = trimmed.indexOf(' ');
        String head = space < 0 ? trimmed : trimmed.substring(0, space);
        return head.toLowerCase();
    }

    public static boolean isObservationVerb(String verb) {
        return "state".equals(verb) || "profile".equals(verb);
    }

    private static boolean containsWhitespaceOrControl(String value) {
        for (int index = 0; index < value.length(); index += 1) {
            char character = value.charAt(index);
            if (Character.isWhitespace(character) || Character.isISOControl(character)) {
                return true;
            }
        }
        return false;
    }

    private static String requiredNonemptyString(JsonObject object, String field)
            throws InvalidCommandException {
        JsonElement value = object.get(field);
        if (value == null || !value.isJsonPrimitive() || !value.getAsJsonPrimitive().isString()) {
            throw new InvalidCommandException(field + " must be a nonempty string");
        }
        String text = value.getAsString().trim();
        if (text.isEmpty()) {
            throw new InvalidCommandException(field + " must be a nonempty string");
        }
        return text;
    }
}
