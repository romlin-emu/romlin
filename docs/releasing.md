# Releasing Romlin

Release-please owns version proposals, changelog entries, and stable `vX.Y.Z` tags.
The current workspace version is inherited by all five crates; do not set crate
versions independently. Its manifest records the last released version, rather
than defining a separate application version. Native `mise run dist` still creates
all macOS, Windows and Linux packages and their existing SHA-512 assets. Linux
builds stay on Ubuntu 22.04 and must pass the glibc 2.35 check.

Use conventional commits (`feat:`, `fix:`, `feat!:`) after the configured bootstrap
commit. Release-please opens a version PR on main. Review the generated changelog
and the workspace/lockfile changes before merging. Existing upstream changelog
entries and attribution remain intact. Curated text under `Unreleased` is retained;
move relevant prose into the generated release entry during review if it belongs
in those release notes. `VERSION=0.20.0 mise run release-notes` accepts both inherited
Keep a Changelog entries and release-please's H2/H3 headings.

The pinned action uses release-please 17.6.0. The `go` strategy is deliberately used
as its supported changelog-only strategy: no Go files or Go build tool are involved.
`simple` would introduce a redundant version.txt, and the stock Rust strategy does
not handle this virtual wildcard workspace's inherited version correctly. Explicit
TOML extra-file updates change `workspace.package.version` and the five named local
packages in Cargo.lock. The `.name.value` filters match the pinned updater's tagged
TOML parser. Run the offline preview when upgrading the action/updater or adding a
workspace member; it checks that all other lockfile bytes and inherited versions
survive unchanged.

After merging the release PR, the same workflow creates a **draft** release and
forces creation of its tag at the merged commit. It checks the tag against the
action's emitted SHA, resolves that commit once, validates its workspace version,
and passes that SHA to CI, packaging and all six smoke environments. Only after
all checks pass does it upload assets to the draft and publish it as latest. CI
also runs on main pushes through Nightly, and directly on PRs and manual runs. No PAT or tag-trigger
side effect is required. GitHub's setting allowing Actions to create PRs must be
enabled by a repository administrator; this setup does not change that setting.
Release PRs made with GITHUB_TOKEN may not trigger PR CI; exact-source CI after
merge is always required by the publication workflow.

If checks, packaging, smoke or upload fail, leave the draft and tag in place.
Re-run failed jobs, or manually run **Release** with the existing `vX.Y.Z` tag.
Manual recovery runs the full exact-source CI/build/smoke pipeline and explicitly
reuses its packages, without a duplicate smoke build. A partial upload is replaced
with `--clobber` while still a draft. An already published release is left alone;
a moved tag or version mismatch fails before publication. Existing manually pushed
`v*` tags still enter the same draft/check/publish path. This pipeline applies to
commits containing this release tooling; historical tags predating it cannot be
rebuilt through the new workflow without backporting the tooling.

Nightlies continue on every main push and manual run, with exact-source CI required
before publication. Main push CI runs once through Nightly. A newer push cancels
the checks and builds of the nightly it supersedes, which also skips that run's
publication; pushes and PRs that change only Markdown, `docs/` or licenses run no
CI or nightly. The publication job is serialized with automatic cancellation
disabled so a newer run cannot interrupt its delete/create sequence. GitHub concurrency
retains at most one pending publication; nightly is a rolling channel, so superseded
pending publications can still be coalesced. Publication requires the main branch
and rechecks its current commit before replacement, so an older build finishing
late cannot replace a newer nightly. Manual runs on other branches only validate
and build. Release notes contain the cocogitto changelog since the newest `v*`
tag reachable from the built commit; `cog.toml` skips merge commits, whose branch
commits already appear individually. Delete/create is still non-atomic if the
service fails or a person cancels publication.

## Local checks

The release checks need Python 3.11+, Node 22+ with npm, and Bash, alongside the
Rust and lint tools used by `mise run check`; `mise install` provides them, including Python and Node. Install the exact preview dependency
into a temporary directory (no repository or release writes), then run the checks:

```sh
npm install --prefix /tmp/romlin-release-preview --ignore-scripts --no-audit --no-fund release-please@17.6.0
NODE_PATH=/tmp/romlin-release-preview/node_modules mise run release-check
NODE_PATH=/tmp/romlin-release-preview/node_modules mise run check
python3 -m unittest discover -s packaging/steamos -v
```

The preview builds a real release-please proposal using a synthetic commit and
applies its actual updaters in memory. Recovery tests fake remote commands; they
never create a PR, release, tag or upload. Local checks do not replace the hosted
three-platform CI, packaging or six-environment gameplay matrix.
