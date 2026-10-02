#!/usr/bin/env node

// The former launcher discovered Java PIDs by set difference and killed them
// on stale bridge state. That is neither process identity nor launch ownership.
// Fail before spawning, enumerating, killing, acquiring, or restarting anything.
function main() {
  throw new Error(
    "Automatic game watchdog is disabled: launch the game/bridge explicitly; " +
    "an interrupted capture requires review, not PID discovery or automatic recovery.",
  );
}

if (require.main === module) {
  try { main(); } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}

module.exports = { main };
