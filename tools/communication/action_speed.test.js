const assert = require("assert");
const { actionTimings } = require("./action_speed");

const rows = [
  {
    type: "command_accept",
    accepted_at: "2026-07-24T13:00:00.000Z",
    command: "PLAY 1 0",
  },
  {
    type: "command_accept",
    accepted_at: "2026-07-24T13:00:00.100Z",
    command: "STATE",
  },
  {
    type: "command_accept",
    accepted_at: "2026-07-24T13:00:00.200Z",
    command: "STATE",
  },
  {
    type: "command_accept",
    accepted_at: "2026-07-24T13:00:01.000Z",
    command: "END",
  },
  {
    type: "command_accept",
    accepted_at: "2026-07-24T13:00:01.500Z",
    command: "CHOOSE 0",
  },
];

const timings = actionTimings(rows.map((row) => JSON.stringify(row)).join("\n"));
assert.deepStrictEqual(
  timings.map(({
    command,
    gap_ms,
    state_polls,
    first_poll_ms,
    settle_poll_span_ms,
    post_poll_ms,
    rolling_aps_10,
  }) => ({
    command,
    gap_ms,
    state_polls,
    first_poll_ms,
    settle_poll_span_ms,
    post_poll_ms,
    rolling_aps_10,
  })),
  [
    {
      command: "PLAY 1 0",
      gap_ms: null,
      state_polls: 0,
      first_poll_ms: null,
      settle_poll_span_ms: 0,
      post_poll_ms: null,
      rolling_aps_10: null,
    },
    {
      command: "END",
      gap_ms: 1000,
      state_polls: 2,
      first_poll_ms: 100,
      settle_poll_span_ms: 100,
      post_poll_ms: 800,
      rolling_aps_10: 1,
    },
    {
      command: "CHOOSE 0",
      gap_ms: 500,
      state_polls: 0,
      first_poll_ms: null,
      settle_poll_span_ms: 0,
      post_poll_ms: null,
      rolling_aps_10: 1.333,
    },
  ],
);

const partial = `${JSON.stringify(rows[0])}\n{"type":"command_accept"`;
assert.strictEqual(actionTimings(partial).length, 1);

console.log("action_speed tests passed");
