const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const net = require('node:net');
const { spawn } = require('node:child_process');
const { test } = require('node:test');
const { validateCollectionStartup } = require('./random_fidelity_collector');

const identity = { status: 'waiting', client_pid: 123, trace_path: '/tmp/test-trace', control: { protocol: 'tcp-jsonl', host: '127.0.0.1', port: 1234 } };
test('collection refuses stale ownership, pending work, nonlocal and mismatched bridges', () => {
  validateCollectionStartup(identity);
  for (const patch of [{ controller: {} }, { pending_command: true }, { command_in_flight: {} }, { summary: { in_game: true } }, { status: 'exited' }, { control: { ...identity.control, host: 'example.com' } }]) {
    assert.throws(() => validateCollectionStartup({ ...identity, ...patch }));
  }
  const live = { ok: true, protocol: 'sts-bridge-jsonl-v1', client_pid: 123, trace_path: identity.trace_path };
  validateCollectionStartup(identity, live);
  for (const patch of [{ client_pid: 124 }, { trace_path: '/other' }, { protocol: 'other' }, { controller: {} }, { pending_command: true }]) {
    assert.throws(() => validateCollectionStartup(identity, { ...live, ...patch }));
  }
});

test('actual collector stops after accepted invalid completion without resend, takeover, abandon or release', { timeout: 15000 }, async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'collector-fail-closed-'));
  const trace = path.join(dir, 'raw.jsonl');
  fs.writeFileSync(trace, JSON.stringify({ type: 'metadata', schema: 7, source: 'communication_mod' }) + '\n');
  let seq = 1, step = 0;
  let pending = false;
  const requests = [];
  const menu = {
    in_game: false, ready_for_command: true, available_commands: ['start_verify', 'profile', 'state'],
    boundary_schema: 7, boundary_kind: 'poll', game_update_seq: 1, dungeon_update_seq: 0,
    actions_queued: 0, card_queue_size: 0, pre_turn_actions_size: 0, current_action: null,
    effects_size: 0, top_level_effects_size: 0, queued_top_level_effects_size: 0, queued_effects_size: 0,
    end_turn_queued: false, command_execution_seq: 0, command_settlement_seq: 0,
    command_response_kind: 'poll', command_response_id: 'initial', transaction_pending: false,
  };
  const state = () => ({ ok: true, protocol: 'sts-bridge-jsonl-v1', client_pid: 123, trace_path: trace,
    step, state_seq: seq, state_id: `s${seq}`, pending_command: pending, summary: menu });
  const server = net.createServer(socket => {
    let input = '';
    socket.on('error', () => {});
    socket.on('data', chunk => {
      input += chunk;
      if (!input.includes('\n')) return;
      const request = JSON.parse(input.split('\n')[0]);
      requests.push(request);
      let response = state();
      if (request.type === 'acquire') response = { ok: true, owner_token: 'fixture-owner' };
      if (request.type === 'command') {
        const oldStep = step;
        step += 1; seq += 1;
        response = { ok: true, step: oldStep, observed_update: state() };
        response.observed_update.summary = { ...menu, command_response_id: request.command_id };
        if (request.command === 'PROFILE') {
          response.observed_update.summary = { type: 'profile', profile: { note_upgrades: 0, final_act_available: true } };
        }
        if (request.command.startsWith('START_VERIFY ')) {
          pending = true;
          fs.appendFileSync(trace, JSON.stringify({ type: 'action', step, command: request.command }) + '\n');
          // A poll cannot settle START_VERIFY. Failure must preserve the pending owner.
          response.observed_update.summary = { ...menu, in_game: true };
        }
      }
      socket.end(JSON.stringify(response) + '\n');
    });
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    fs.writeFileSync(path.join(dir, 'status.json'), JSON.stringify({ ...identity, trace_path: trace,
      control: { ...identity.control, port: server.address().port } }));
    fs.writeFileSync(path.join(dir, 'prefs.json'), '{}');
    const child = spawn(process.execPath, [path.join(__dirname, 'random_fidelity_collector.js')], {
      env: { ...process.env, STS_BRIDGE_SESSION_DIR: dir, STS_SEEN_BOSSES_PATH: path.join(dir, 'prefs.json'),
        STS_RANDOM_OUTPUT_DIR: path.join(dir, 'output'), STS_GAME_SEED: 'FAILCLOSED', STS_RANDOM_POLICY_SEED: '1' },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let logs = '';
    child.stdout.on('data', b => { logs += b; }); child.stderr.on('data', b => { logs += b; });
    const code = await new Promise((resolve, reject) => { child.on('error', reject); child.on('exit', resolve); });
    assert.notEqual(code, 0, logs);
    assert.equal(requests.filter(r => r.command?.startsWith('START_VERIFY ')).length, 1, logs);
    assert.equal(pending, true);
    assert.equal(requests.some(r => ['release', 'abandon_run'].includes(r.type)), false);
    const acquire = requests.find(r => r.type === 'acquire');
    assert.equal(acquire.takeover_if_stale_after_ms, undefined);
    assert.equal(acquire.cancel_orphaned_command_after_ms, undefined);
    assert.match(fs.readFileSync(trace, 'utf8'), /START_VERIFY/);
  } finally {
    await new Promise(resolve => server.close(resolve));
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
