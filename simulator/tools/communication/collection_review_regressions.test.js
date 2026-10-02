const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const net = require('node:net');
const { spawnSync } = require('node:child_process');
const { test } = require('node:test');
const { controlRequest, send, requireCommandCompletion } = require('./random_fidelity_collector');
const { firstUncollectedPolicySeed } = require('./run_random_fidelity_campaign');
const summary = {
  in_game: true, ready_for_command: true, boundary_schema: 7, boundary_kind: 'quiescent',
  game_update_seq: 5, dungeon_update_seq: 5, actions_queued: 0, card_queue_size: 0,
  pre_turn_actions_size: 0, current_action: null, end_turn_queued: false,
  effects_size: 0, top_level_effects_size: 0, queued_top_level_effects_size: 0, queued_effects_size: 0,
  command_execution_seq: 6, command_settlement_seq: 6,
  command_response_kind: 'settled', command_response_id: 'sent-id', transaction_pending: false,
};
const accepted = { commandId: 'sent-id', executionSeq: 5, settlementSeq: 5 };
const completion = patch => ({ ok: true, step: 2, state_seq: 11, summary: { ...summary, ...patch } });

test('control requests close locally after one complete response', async () => {
  let connectionClosed;
  const closed = new Promise(resolve => { connectionClosed = resolve; });
  const server = net.createServer(socket => {
    socket.on('error', () => {});
    socket.on('close', connectionClosed);
    socket.on('data', () => socket.write('{"ok":true}\n'));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    assert.deepEqual(
      await controlRequest({ host: '127.0.0.1', port: server.address().port }, { type: 'state' }),
      { ok: true },
    );
    await Promise.race([
      closed,
      new Promise((_, reject) => setTimeout(() => reject(new Error('client socket stayed open')), 250)),
    ]);
  } finally {
    await new Promise(resolve => server.close(resolve));
  }
});

test('completion requires exact response identity and +1 execution/settlement fences', () => {
  requireCommandCompletion(completion({}), 'CHOOSE 0', 2, 10, accepted);
  for (const patch of [
    { command_response_id: 'other-id' }, { command_response_kind: 'unsolicited', command_response_id: null },
    { command_execution_seq: 7 }, { command_settlement_seq: 7 },
    { command_execution_seq: 5 }, { command_settlement_seq: 5 }, { transaction_pending: true },
  ]) assert.throws(() => requireCommandCompletion(completion(patch), 'CHOOSE 0', 2, 10, accepted));
  const poll = { boundary_kind: 'poll', command_response_kind: 'poll', command_execution_seq: 5, command_settlement_seq: 5 };
  requireCommandCompletion(completion(poll), 'STATE', 2, 10, accepted);
  assert.throws(() => requireCommandCompletion(completion({ ...poll, command_execution_seq: 6 }), 'STATE', 2, 10, accepted));
  requireCommandCompletion({ ok: true, step: 2, state_seq: 11, summary: { type: 'profile', profile: {} } }, 'PROFILE', 2, 10, accepted);
});

test('accepted timeout cannot be replaced by a later cached state', async () => {
  const requests = [];
  const server = net.createServer(socket => {
    let input = '';
    socket.on('error', () => {});
    socket.on('data', data => {
      input += data;
      if (!input.includes('\n')) return;
      const request = JSON.parse(input.split('\n')[0]); requests.push(request);
      const reply = request.type === 'command'
        ? { ok: true, step: 1, observed_update: { ok: false, error: 'observation timeout' } }
        : { ...completion({ command_response_id: 'unrelated-id' }), pending_command: false };
      socket.end(JSON.stringify(reply) + '\n');
    });
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    await assert.rejects(send({ host: '127.0.0.1', port: server.address().port }, 'owner', {
      step: 1, state_seq: 10, state_id: 'source',
      summary: { ...summary, command_execution_seq: 5, command_settlement_seq: 5 },
    }, 'CHOOSE 0', {}), /after acceptance|deadline/);
    assert.deepEqual(requests.map(r => r.type), ['command']);
  } finally { await new Promise(resolve => server.close(resolve)); }
});

test('a matching first metadata line and incomplete ledger never seal a policy seed', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'campaign-partial-'));
  try {
    fs.mkdirSync(path.join(dir, 'traces'));
    const trace = path.join(dir, 'traces/FIDL00001-p1-partial.jsonl');
    fs.writeFileSync(trace, JSON.stringify({ type: 'metadata', collection: { policy_seed: 1, game_seed: 'FIDL00001' } }) + '\n');
    assert.throws(() => firstUncollectedPolicySeed(dir, 'FIDL'), /unsealed trace/);
    fs.writeFileSync(path.join(dir, 'ledger.jsonl'), JSON.stringify({ kind: 'collected_incomplete', policy_seed: 1, game_seed: 'FIDL00001', trace }) + '\n');
    assert.throws(() => firstUncollectedPolicySeed(dir, 'FIDL'), /incomplete capture/);
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
});

test('malformed campaign limits cannot report a zero-run success', () => {
  for (const value of ['garbage', '-1', '1junk', '', '1.5', '9007199254740992']) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'campaign-limit-'));
    try {
      const run = spawnSync(process.execPath, [path.join(__dirname, 'run_random_fidelity_campaign.js')], {
        env: { ...process.env, STS_RANDOM_MAX_RUNS: value, STS_RANDOM_OUTPUT_DIR: dir, STS_BRIDGE_SESSION_DIR: path.join(dir, 'absent') }, encoding: 'utf8', timeout: 3000,
      });
      assert.notEqual(run.status, 0, value);
      assert.match(run.stderr, /STS_RANDOM_MAX_RUNS/, value);
    } finally { fs.rmSync(dir, { recursive: true, force: true }); }
  }
});

test('campaign stops on its first child failure, even in indefinite mode, and requires sealed success', () => {
  for (const code of [9, 0]) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'campaign-child-'));
    try {
      fs.copyFileSync(path.join(__dirname, 'run_random_fidelity_campaign.js'), path.join(dir, 'run_random_fidelity_campaign.js'));
      fs.writeFileSync(path.join(dir, 'random_fidelity_collector.js'), `
        module.exports = require(${JSON.stringify(path.join(__dirname, 'random_fidelity_collector.js'))});
        if (require.main === module) {
          require('fs').appendFileSync(require('path').join(process.env.STS_RANDOM_OUTPUT_DIR, 'calls'), 'called\\n');
          process.exit(${code});
        }
      `);
      const run = spawnSync(process.execPath, [path.join(dir, 'run_random_fidelity_campaign.js')], {
        env: { ...process.env, STS_RANDOM_MAX_RUNS: code === 9 ? '0' : '1', STS_RANDOM_POLICY_SEED: '1',
          STS_RANDOM_OUTPUT_DIR: dir, STS_RANDOM_RETRY_DELAY_MS: '5', STS_RANDOM_MAX_RETRY_DELAY_MS: '10' },
        encoding: 'utf8', timeout: 3000,
      });
      assert.equal(run.error, undefined, run.stderr);
      assert.notEqual(run.status, 0);
      assert.equal(fs.readFileSync(path.join(dir, 'calls'), 'utf8'), 'called\n');
      assert.notEqual(JSON.parse(fs.readFileSync(path.join(dir, 'campaign_status.json'))).status, 'complete');
      const lock = fs.readFileSync(path.join(dir, 'campaign.lock'), 'utf8');
      const repeat = spawnSync(process.execPath, [path.join(dir, 'run_random_fidelity_campaign.js')], {
        env: { ...process.env, STS_RANDOM_MAX_RUNS: '1', STS_RANDOM_POLICY_SEED: '1', STS_RANDOM_OUTPUT_DIR: dir },
        encoding: 'utf8', timeout: 3000,
      });
      assert.notEqual(repeat.status, 0);
      assert.match(repeat.stderr, /EEXIST/);
      assert.equal(fs.readFileSync(path.join(dir, 'calls'), 'utf8'), 'called\n');
      assert.equal(fs.readFileSync(path.join(dir, 'campaign.lock'), 'utf8'), lock);
    } finally { fs.rmSync(dir, { recursive: true, force: true }); }
  }
});

test('a sealed successful batch alone releases its exact campaign lock', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'campaign-success-'));
  try {
    fs.copyFileSync(path.join(__dirname, 'run_random_fidelity_campaign.js'), path.join(dir, 'run_random_fidelity_campaign.js'));
    // Synthetic transport/sealing fixture; not real-game parity evidence.
    fs.writeFileSync(path.join(dir, 'random_fidelity_collector.js'), `
      module.exports = require(${JSON.stringify(path.join(__dirname, 'random_fidelity_collector.js'))});
      if (require.main === module) {
        const fs = require('fs'), path = require('path'), crypto = require('crypto');
        const directory = process.env.STS_RANDOM_OUTPUT_DIR;
        const policy = Number(process.env.STS_RANDOM_POLICY_SEED), seed = process.env.STS_GAME_SEED;
        fs.mkdirSync(path.join(directory, 'traces'), {recursive: true});
        const trace = path.join(directory, 'traces', seed + '-p' + policy + '-test.jsonl');
        const content = JSON.stringify({type: 'metadata', collection: {policy_seed: policy, game_seed: seed}}) + '\\n';
        fs.writeFileSync(trace, content);
        fs.appendFileSync(path.join(directory, 'ledger.jsonl'), JSON.stringify({kind: 'collected', terminal_reason: 'game_over',
          policy_seed: policy, game_seed: seed, trace, trace_sha256: crypto.createHash('sha256').update(content).digest('hex')}) + '\\n');
      }
    `);
    const run = spawnSync(process.execPath, [path.join(dir, 'run_random_fidelity_campaign.js')], {
      env: { ...process.env, STS_RANDOM_MAX_RUNS: '2', STS_RANDOM_POLICY_SEED: '1', STS_RANDOM_GAME_SEED_PREFIX: 'TEST', STS_RANDOM_OUTPUT_DIR: dir },
      encoding: 'utf8', timeout: 3000,
    });
    assert.equal(run.status, 0, run.stderr);
    assert.equal(JSON.parse(fs.readFileSync(path.join(dir, 'campaign_status.json'))).captured_runs, 2);
    assert.equal(fs.existsSync(path.join(dir, 'campaign.lock')), false);
    assert.equal(fs.readFileSync(path.join(dir, 'ledger.jsonl'), 'utf8').trim().split('\n').length, 2);
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
});

test('invalid collector limits are rejected before reading a bridge or acquiring ownership', () => {
  for (const variable of ['STS_RANDOM_MAX_ACTIONS', 'STS_RANDOM_POLICY_SEED', 'STS_STARTING_HP']) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'collector-limit-'));
    try {
      const run = spawnSync(process.execPath, [path.join(__dirname, 'random_fidelity_collector.js')], {
        env: { ...process.env, [variable]: 'NaN', STS_RANDOM_OUTPUT_DIR: dir, STS_BRIDGE_SESSION_DIR: path.join(dir, 'absent') }, encoding: 'utf8', timeout: 3000,
      });
      assert.notEqual(run.status, 0);
      assert.match(run.stderr, new RegExp(variable));
    } finally { fs.rmSync(dir, { recursive: true, force: true }); }
  }
});
