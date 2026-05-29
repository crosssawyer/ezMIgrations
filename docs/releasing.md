# Releasing

This is the only doc you need to cut a new release of ezMigrations. The CI does
the heavy lifting; your job is to bump the version in the four places below,
tag, and watch the run go green.

## TL;DR (the happy path)

You want to cut `v1.2.0`. From a clean tree:

1. Bump the version to `1.2.0` in all four files (see [Version field locations](#version-field-locations)):

   ```bash
   # package.json            -> "version": "1.2.0"
   # src-tauri/tauri.conf.json -> "version": "1.2.0"
   # src-tauri/Cargo.toml    -> version = "1.2.0"
   # src-tauri/Cargo.lock    -> the [[package]] entry for "ez-migrations"
   ```

   The Cargo.lock entry updates itself if you run any cargo command after
   editing Cargo.toml, e.g.:

   ```bash
   cd src-tauri && cargo check
   ```

   Also add a `## [1.2.0] - <date>` section to `CHANGELOG.md`. The release
   workflow uses that section verbatim as the GitHub Release body, so write
   it before you tag (see [What the pipeline does](#what-the-pipeline-does)).

2. Commit on `nightly`:

   ```bash
   git checkout nightly
   git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
   git commit -m "chore(release): bump to 1.2.0 for v1.2.0"
   ```

3. Fast-forward `main` to `nightly`:

   ```bash
   git checkout main
   git merge --ff-only nightly
   git push origin main
   ```

4. Tag and push:

   ```bash
   git tag -a v1.2.0 -m "Release v1.2.0"
   git push origin v1.2.0
   ```

5. Open the Actions tab in GitHub and watch the `Release` workflow. When the
   matrix job goes green on all three OSes, the release shows up on the
   Releases page with installers attached.

## Version field locations

The version string lives in four files and they must all match. If they
drift, the bundlers will fail or produce installers that disagree about what
version they are:

- `package.json` — `"version"` field
- `src-tauri/tauri.conf.json` — top-level `"version"` field
- `src-tauri/Cargo.toml` — `[package]` `version` field
- `src-tauri/Cargo.lock` — the `[[package]]` block whose `name = "ez-migrations"`

`Cargo.lock` is tracked in the repo on purpose (see [Cargo.lock is tracked](#cargolock-is-tracked-abd9f5a)).
Don't try to remove it from the bump.

## Versioning conventions

- **Prod releases** ship from `main` with tag `vX.Y.Z` (e.g. `v1.1.0`).
  The internal version in all four files is `X.Y.Z`.
- **Nightly / pre-release builds** ship from `nightly` with tag
  `vX.Y.Z-nightly` (e.g. `v1.1.0-nightly`). The git tag is allowed to use the
  `-nightly` suffix because it's just a name, but the **internal version
  string in the four files must use a numeric pre-release id**: `X.Y.Z-1`,
  `X.Y.Z-2`, etc. Bump the trailing number for each new nightly.

  Why: the Windows MSI bundler rejects alphabetic pre-release ids and only
  accepts numeric ones ≤ 65535. See [Windows MSI pre-release id format](#windows-msi-pre-release-id-format-acb14ac).

  Recent examples in the history:
  - `b4b0bd0 chore(release): bump to 1.1.0-1 for v1.1.0-nightly`
  - `acb14ac fix(release): use numeric pre-release id for Windows MSI bundler`

## What the pipeline does

The pipeline is `.github/workflows/release.yml`. There are two jobs.

### Job 1: `create-release` (ubuntu-latest)

A tiny coordinator job that exists so the matrix builds agree on what tag
they're publishing under.

- Checks out the repo.
- Computes `tag`:
  - On `push` to a `v*` tag, `tag = github.ref_name` (the tag you pushed).
  - On `workflow_dispatch`, `tag = "v" + inputs.version`.
- On `workflow_dispatch` only, it also creates the git tag and pushes it
  (with `|| true` so an already-existing tag does not fail the job — see
  [03f5d27](#release-yml-history)).
- Outputs: `tag`, consumed by the build matrix.

### Job 2: `build-and-release` (matrix)

Builds the app and uploads installers to the GitHub Release. Matrix:

| OS runner       | Rust target                  | Bundle outputs                |
| --------------- | ---------------------------- | ----------------------------- |
| `ubuntu-latest` | `x86_64-unknown-linux-gnu`   | `.deb`, `.AppImage`           |
| `windows-latest`| `x86_64-pc-windows-msvc`     | `.msi`, `.exe` (NSIS)         |
| `macos-latest`  | `aarch64-apple-darwin`       | `.dmg` (Apple Silicon)        |

`fail-fast: false` is set, so one OS failing does not cancel the others.

Steps per matrix entry:

1. Checkout.
2. **Linux only** — `apt-get install` the WebKitGTK 4.1 stack:
   `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`.
3. Set up Node 20 (`actions/setup-node@v4`).
4. Set up the stable Rust toolchain for the matrix target (`dtolnay/rust-toolchain@stable`).
5. Restore the Rust cache for the `src-tauri` workspace (`swatinem/rust-cache@v2`).
6. `npm ci` to install frontend deps.
7. **`tauri-apps/tauri-action@v0`** — this is what does everything else:
   - Runs `npm run build` (the `beforeBuildCommand` in `tauri.conf.json`)
     to produce the Vite output in `dist/`.
   - Runs `npx tauri build`, which bundles the Rust binary plus the frontend
     into platform installers.
   - Creates the GitHub Release (named `ezMigrations <tag>`, not a draft).
     The body is the `## [X.Y.Z]` section of `CHANGELOG.md` matching the
     tag's version, extracted by the `Extract release notes from CHANGELOG`
     step in the `create-release` job. If no matching section exists (e.g. a
     nightly whose version isn't in the changelog), it falls back to `"See
     the assets below to download the installer for your platform."` So:
     **add the changelog section before you tag**, or the notes fall back to
     the generic line. The release is marked as a pre-release iff the tag
     contains a `-` (e.g. `v1.1.0-1`, `v1.2.0-nightly`); plain `vX.Y.Z` tags
     publish as full releases.
   - Uploads the built installers as release assets.

Inputs / secrets:

- `GITHUB_TOKEN` from `secrets.GITHUB_TOKEN` (auto-provided; the workflow
  declares `permissions: contents: write` so it can create releases).
- No code-signing secrets are configured. macOS and Windows installers are
  unsigned today.

### Bundle targets produced

`tauri.conf.json` has `bundle.targets: "all"`, which on the runners above
means:

- Windows: `.msi` (WiX) and `.exe` (NSIS)
- macOS (aarch64): `.dmg` (and the underlying `.app`)
- Linux: `.deb` and `.AppImage`

No `x86_64-apple-darwin` (Intel mac) matrix entry exists, so there is no
Intel-mac installer.

## Triggering a release

There are two supported paths.

### Tag push (recommended)

This is what the TL;DR shows. The summary:

```bash
git tag -a v1.2.0 -m "Release v1.2.0"
git push origin v1.2.0
```

The `on.push.tags: ["v*"]` filter catches it. The tag name **is** the release
name and must start with `v`.

### Manual workflow dispatch

Use this if you need to re-run the pipeline against an existing version, or
to publish from a branch that already has the right version bumped in but
where you don't want to create the tag locally first.

1. GitHub UI -> Actions -> `Release` workflow -> `Run workflow`.
2. Enter the version **without** the `v` prefix, e.g. `1.2.0`.
3. The job will create and push `v1.2.0` for you (idempotent if it already
   exists).

Note: workflow_dispatch reads the version string from the input but it does
*not* edit the four version files for you. They still have to be bumped on
the branch the dispatch runs against, or the resulting installers will carry
the old version.

## Nightly builds

Nightly builds use the same workflow — there is no separate nightly pipeline.

- Branch: `nightly`.
- Tag: `vX.Y.Z-nightly` (e.g. `v1.1.0-nightly`).
- Internal version: `X.Y.Z-N` where `N` is a monotonically increasing
  number (`1.1.0-1`, `1.1.0-2`, …). See [Windows MSI pre-release id format](#windows-msi-pre-release-id-format-acb14ac).
- The workflow auto-detects pre-releases: any tag containing a `-` (so
  `v1.1.0-nightly`, `v1.1.0-1`, `v1.2.0-rc1`, etc.) is published with
  `prerelease: true`. Plain `vX.Y.Z` tags publish as full releases.

## Lessons learned / footguns

### Windows MSI pre-release id format (`acb14ac`)

**Symptom:** Release workflow goes red on `windows-latest` with the MSI
bundler complaining about the version string. Versions like `1.0.0-nightly`
or `1.0.0-foo` are rejected.

**Cause:** The WiX MSI target only accepts a numeric pre-release id ≤ 65535.
Alphabetic ids like `nightly` blow up. The dmg, deb, and AppImage bundlers
all accept the alphabetic form, so this looks like a Windows-only crash
until you trace it.

**Fix:** Use `X.Y.Z-N` (numeric `N`) as the internal version in the four
files. Keep the human-readable `-nightly` suffix in the git tag and the
release title only.

### Windows PATH corruption when shelling to `dotnet ef` (`1abe332`)

**Symptom:** On Windows builds of the released app, the migrations list
silently comes back empty. No error toast — just nothing there.

**Cause:** `src-tauri/src/dotnet.rs` used to enrich `PATH` with macOS-style
install paths joined with `:` on every platform. On Windows, `PATH` uses `;`
as the separator, so the injected blob collapsed the first real entry
(typically `C:\Program Files\dotnet`) into an unparseable string. `dotnet ef
migrations list` then ran against whatever happened to be next in PATH (or
nothing), produced output the parser didn't recognize, and the UI showed an
empty list.

**Why it matters for releases specifically:** in `npm run tauri dev` you
inherit the developer's already-correct `PATH`, so the bug is masked. The
installed release build sees a stock user `PATH` and the corruption goes
unnoticed until the migrations list is empty.

**Fix:** The PATH enrichment is now gated to non-Windows platforms.
Re-verify on a clean Windows VM after any change to `dotnet.rs` (see
[Verifying a release](#verifying-a-release)).

### `Cargo.lock` is tracked (`abd9f5a`)

**Symptom:** Reproducibility drift. Two CI runs days apart produced subtly
different binaries because transitive dep bumps landed silently.

**Fix:** `src-tauri/Cargo.lock` is committed (it was removed from
`.gitignore` in `abd9f5a`). Tauri apps are binary crates, so the Rust
convention is to track the lockfile. **Do not delete it.** When you bump
the four version files, the Cargo.lock entry for `ez-migrations` must move
too — run `cargo check` (or `cargo update -p ez-migrations`) in `src-tauri`
after editing `Cargo.toml` to keep them in sync.

### `tauri-action` owns release creation (`5189c1d`)

**History:** An older version of the workflow built artifacts in the matrix
and then a separate job tried to download and upload them to the release.
That job kept failing to locate the built files because the paths differed
per OS.

**Current approach:** `tauri-apps/tauri-action@v0` runs in each matrix entry
and uploads its own outputs directly to the release. Don't reintroduce a
separate upload step; if assets aren't appearing on a release, look at the
`Build and release Tauri app` step's logs first.

### Tag already exists on re-run (`03f5d27`)

**Symptom:** Re-running the workflow against an existing tag failed at the
`git tag` step because the tag was already on the remote.

**Fix:** The `Create tag (manual dispatch only)` step has `|| true` after
both the `git tag` and `git push`. You can safely re-dispatch the workflow
against an existing version.

### `release.yml` history

If you need to dig further:

```bash
git log --oneline -- .github/workflows/release.yml
```

Key commits: `c025663` (initial Tauri rewrite of the pipeline), `5189c1d`
(switch to tauri-action), `03f5d27` (idempotent tag creation).

## Verifying a release

When the workflow goes green, don't trust it — the only thing CI confirms is
that the bundlers didn't crash. Manually verify:

1. From the Releases page, download every installer for the platforms you
   have access to:
   - Windows: `.msi` **and** `.exe`
   - macOS (Apple Silicon): `.dmg`
   - Linux: `.deb` **and** `.AppImage`
2. Install on a clean machine or VM for each OS (a clean Windows VM in
   particular — see the PATH footgun above; that bug only shows in
   non-developer environments).
3. Launch the app. Confirm the title bar / About says the right version.
4. Add a project pointing at a real EF Core migrations project.
5. Confirm the migrations list populates (this exercises `dotnet ef
   migrations list` and the PATH handling).
6. Run **one** end-to-end migration create + delete to confirm `dotnet ef`
   resolution works for write operations, not just reads.

If any of these fail on a platform, treat the release as bad and see
[Rolling back](#rolling-back).

## Rolling back

If the release is broken after publishing:

1. Open the release on the Releases page and click **Edit**. Tick **Set as
   a pre-release** so users browsing the page see the warning and don't
   auto-update into a bad build.
2. If the installers are actively harmful, click **Delete release** to take
   the assets down. This does **not** delete the git tag.
3. To delete the tag too:

   ```bash
   git push origin :refs/tags/v1.2.0   # remove from remote
   git tag -d v1.2.0                   # remove locally
   ```

4. Fix the underlying issue on `nightly`, ship a nightly to verify, then
   re-cut the same version (e.g. push `v1.2.0` again after the fix lands on
   `main`) or bump the patch number to `v1.2.1` and start over from the
   TL;DR.

Prefer re-tagging at the next patch version over re-pushing the same tag —
re-tagging is less confusing for anyone who already downloaded the bad
installer.
