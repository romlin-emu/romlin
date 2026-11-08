// Offline preview using the exact library bundled by release-please-action v5.0.0.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {buildStrategy} = require('release-please/build/src/factory');
const {parseConventionalCommits} = require('release-please/build/src/commit');
const {Manifest} = require('release-please/build/src/manifest');
const {GitHubApi} = require('release-please/build/src/github-api');
const {Version} = require('release-please/build/src/version');
const {TagName} = require('release-please/build/src/util/tag-name');
const {ReleasePleaseManifest} = require('release-please/build/src/updaters/release-please-manifest');

(async () => {
  assert.equal(require('release-please/package.json').version, '17.6.0');
  const config = JSON.parse(fs.readFileSync('release-please-config.json'));
  const oldVersion = JSON.parse(fs.readFileSync('.release-please-manifest.json'))['.'];
  const current = Version.parse(oldVersion);
  const next = new Version(current.major, current.minor + 1, 0);
  const github = {
    repository: {owner: 'romlin-emu', repo: 'romlin', defaultBranch: 'main'},
    getFileJson: async file => JSON.parse(fs.readFileSync(file)),
  };
  const parsed = await Manifest.fromManifest(github, 'main');
  const options = parsed.repositoryConfig['.'];
  assert.equal(options.draft, true);
  assert.equal(options.forceTag, true);
  assert.equal(options.includeVInTag, true);
  assert.equal(options.includeComponentInTag, false);
  const strategy = await buildStrategy({...options, github, targetBranch: 'main', path: '.'});
  const preview = await strategy.buildReleasePullRequest(
    await parseConventionalCommits([{sha: 'a'.repeat(40), message: 'feat: offline release preview', files: ['Cargo.toml']}]),
    {tag: new TagName(Version.parse(oldVersion)), sha: config['bootstrap-sha'], notes: ''},
    false, [], {newVersion: next},
  );
  assert.deepEqual(preview.updates.map(u => u.path).sort(), ['CHANGELOG.md', 'Cargo.lock', 'Cargo.toml']);
  const local = ['romlin-app', 'romlin-cheevos', 'romlin-libretro', 'romlin-proto', 'romlin-runner'];
  const outputs = {};
  for (const update of preview.updates) {
    const before = fs.readFileSync(update.path, 'utf8');
    const after = update.updater.updateContent(before);
    outputs[update.path] = after;
    if (update.path === 'Cargo.toml') {
      assert.equal(after, before.replace(`version = "${oldVersion}"`, `version = "${next}"`));
    } else if (update.path === 'Cargo.lock') {
      let changed = 0;
      const expected = before.replace(/\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"/g, (entry, name, version) => {
        if (!local.includes(name)) return entry;
        assert.equal(version, oldVersion);
        changed++;
        return entry.replace(`version = "${version}"`, `version = "${next}"`);
      });
      assert.equal(changed, 5);
      assert(after === expected, 'Only the five local versions may change, byte for byte');
    } else {
      assert(after.includes(before.slice(before.indexOf('## [Unreleased]'))), 'Inherited changelog must survive');
      assert(after.includes('offline release preview'));
    }
  }
  for (const dir of fs.readdirSync('crates')) {
    const manifest = path.join('crates', dir, 'Cargo.toml');
    if (fs.existsSync(manifest)) assert.match(fs.readFileSync(manifest, 'utf8'), /^version.workspace = true$/m);
  }
  const manifest = new ReleasePleaseManifest({version: next, versionsMap: new Map([['.', next]])});
  // Version map update is exercised by the tool, not by editing a fixture ourselves.
  const updatedManifest = manifest.updateContent(fs.readFileSync('.release-please-manifest.json', 'utf8'));
  assert.equal(JSON.parse(updatedManifest)['.'], next.toString());
  const cp = require('node:child_process');
  const parsedNotes = cp.execFileSync(process.env.PYTHON || 'python3', ['-c',
    'import sys; sys.path.insert(0, ".github/release"); from release import notes; print(notes(sys.argv[1], sys.stdin.read()))', next.toString()],
    {input: outputs['CHANGELOG.md'], encoding: 'utf8'});
  assert(parsedNotes.includes('offline release preview'));
  assert(!parsedNotes.includes('Nintendo Switch'));
  // Exercise the pinned tool's actual draft/tag API ordering with offline fakes.
  const calls = [];
  const api = new GitHubApi({repository: github.repository, octokitAPIs: {octokit: {
    git: {createRef: async request => { calls.push(['tag', request]); }},
    repos: {createRelease: async request => {
      calls.push(['release', request]);
      return {data: {id: 1, tag_name: request.tag_name, target_commitish: request.target_commitish, draft: request.draft}};
    }},
  }}});
  await api.createRelease({tag: new TagName(next), sha: 'a'.repeat(40), notes: 'notes'}, options);
  assert.deepEqual(calls.map(call => call[0]), ['tag', 'release']);
  assert.equal(calls[0][1].sha, 'a'.repeat(40));
  assert.equal(calls[0][1].ref, `refs/tags/v${next}`);
  assert.equal(calls[1][1].draft, true);

  const yaml = require('yaml');
  const workflow = file => yaml.parse(fs.readFileSync('.github/workflows/' + file, 'utf8'));
  const release = workflow('release.yml');
  const nightly = workflow('nightly.yml');
  assert.equal(nightly.jobs.publish.if, "github.ref == 'refs/heads/main'");
  const directory = fs.mkdtempSync(path.join(require('node:os').tmpdir(), 'romlin-nightly-'));
  try {
    const log = path.join(directory, 'calls');
    fs.writeFileSync(path.join(directory, 'changelog.md'), 'changelog\n');
    fs.writeFileSync(path.join(directory, 'gh'), '#!/bin/sh\nprintf "%s\\n" "$*" >> "$GH_CALLS"\nif [ "$1" = api ]; then\n  [ "$TEST_HEAD" != fail ] || exit 1\n  printf "%s\\n" "$TEST_HEAD"\nfi\n', {mode: 0o755});
    const script = nightly.jobs.publish.steps.find(step => step.name === 'Replace the nightly release').run;
    for (const head of ['a'.repeat(40), 'b'.repeat(40), 'fail']) {
      fs.writeFileSync(log, '');
      const result = cp.spawnSync('bash', ['-e', '-o', 'pipefail', '-c', script], {encoding: 'utf8', cwd: directory, env: {
        ...process.env, PATH: directory + path.delimiter + process.env.PATH,
        GH_REPO: 'romlin-emu/romlin', GH_CALLS: log, TEST_HEAD: head, GITHUB_SHA: 'a'.repeat(40),
      }});
      assert.equal(result.status, head === 'fail' ? 1 : 0, result.stderr);
      const mutations = fs.readFileSync(log, 'utf8').split('\n').filter(line => line.startsWith('release '));
      assert.equal(mutations.length, head === 'a'.repeat(40) ? 2 : 0, 'Stale or unverifiable builds must not replace nightly');
    }
  } finally {
    fs.rmSync(directory, {recursive: true, force: true});
  }
  assert.equal(release.concurrency['cancel-in-progress'], false);
  assert.equal(nightly.jobs.publish.concurrency['cancel-in-progress'], false);
  for (const job of ['ci', 'build']) assert.equal(nightly.jobs[job].concurrency['cancel-in-progress'], true);
  assert.deepEqual(nightly.jobs.publish.needs, ['ci', 'build']);
  assert.equal(nightly.jobs.ci.with.ref, '${{ github.sha }}');
  assert.deepEqual(release.jobs.publish.needs, ['prepare', 'release-ci', 'build', 'smoke']);
  for (const job of ['release-ci', 'build', 'smoke']) {
    assert.equal(release.jobs[job].with.ref, '${{ needs.prepare.outputs.sha }}');
    assert.deepEqual(release.jobs[job].permissions, {contents: 'read'});
  }
  assert.equal(release.jobs.smoke.with.build, false);
  const smoke = workflow('smoke.yml');
  assert.equal(smoke.on.workflow_call.inputs.build.default, false);
  assert.equal(smoke.on.workflow_dispatch.inputs.build.default, true);
  assert.equal(smoke.jobs.smoke.strategy.matrix.include.length, 6);
  assert.equal(workflow('ci.yml').jobs.check.strategy.matrix.include.length, 3);
  assert(workflow('build.yml').jobs.build.strategy.matrix.include.some(row => row.os === 'ubuntu-22.04'));
  console.log('Offline release preview: workspace, five lock entries, inherited versions, changelog and notes verified');
})().catch(error => { console.error(error); process.exitCode = 1; });
