const os = require("os");
const path = require("path");

function defaultRandomFidelityOutputDir() {
  return path.join(os.homedir(), "sts-traces", "random-fidelity");
}

function resolveRandomFidelityOutputDir(env = process.env) {
  const override = env.STS_RANDOM_OUTPUT_DIR;
  if (typeof override === "string" && override.trim() !== "") {
    return path.resolve(override);
  }
  return defaultRandomFidelityOutputDir();
}

module.exports = {
  defaultRandomFidelityOutputDir,
  resolveRandomFidelityOutputDir,
};
