import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { aggregateArtifacts, checkVersions, platforms, releaseNotes, stageArtifacts } from "./release.mjs";

function fixtures(context) {
  const root = mkdtempSync(join(tmpdir(), "gdom-release-test-"));
  context.after(() => rmSync(root, { recursive: true, force: true }));
  const input = join(root, "input");
  mkdirSync(input);
  for (const [platform, definition] of Object.entries(platforms)) {
    const directory = join(input, platform);
    mkdirSync(directory);
    const suffixes = [definition.suffix, ...(platform.startsWith("darwin-") ? [".dmg"] : [])];
    const artifacts = suffixes.map((suffix) => {
      const filename = `GDOM_0.1.1_${platform}${suffix}`;
      writeFileSync(join(directory, filename), "fixture artifact");
      return { filename, sha256: createHash("sha256").update("fixture artifact").digest("hex") };
    });
    writeFileSync(join(directory, `${artifacts[0].filename}.sig`), "fixture signature");
    writeFileSync(join(directory, "receipt.json"), JSON.stringify({ platform, version: "0.1.1", commit: "fixture-commit", signed: true, artifacts }));
  }
  return { input, output: join(root, "output") };
}
const aggregate = ({ input, output }) => aggregateArtifacts(input, output, "0.1.1", "fixture-commit", "phucrio/gdom", "User-visible fixes.");

test("aggregates six distinct OS and architecture download routes", (context) => {
  const paths = fixtures(context);
  const manifest = aggregate(paths);
  assert.equal(Object.keys(manifest.platforms).length, 6);
  assert.match(manifest.platforms["linux-aarch64"].url, /linux-aarch64.AppImage$/);
  assert.match(manifest.platforms["darwin-x86_64"].url, /darwin-x86_64.app.tar.gz$/);
});
test("refuses a release missing an architecture", (context) => {
  const paths = fixtures(context);
  rmSync(join(paths.input, "windows-aarch64"), { recursive: true });
  assert.throws(() => aggregate(paths), /six platform/);
});
test("refuses changed artifacts after staging", (context) => {
  const paths = fixtures(context);
  writeFileSync(join(paths.input, "linux-aarch64", "GDOM_0.1.1_linux-aarch64.AppImage"), "tampered");
  assert.throws(() => aggregate(paths), /checksum mismatch/);
});
test("refuses missing updater signatures", (context) => {
  const paths = fixtures(context);
  writeFileSync(join(paths.input, "windows-x86_64", "GDOM_0.1.1_windows-x86_64.exe.sig"), "");
  assert.throws(() => aggregate(paths), /Missing updater signature/);
});
test("refuses mixed commit releases", (context) => {
  const paths = fixtures(context);
  assert.throws(() => aggregateArtifacts(paths.input, paths.output, "0.1.1", "other-commit", "phucrio/gdom", "Notes"), /receipt/);
});
test("extracts only the requested dated version and rejects prereleases", () => {
  const changelog = "# Changes\n\n## [Unreleased]\n\nFuture\n\n## [0.1.1] - 2026-09-08\n\nFixed updates.\n\n## [0.1.0] - 2026-09-01\n\nInitial\n";
  assert.equal(releaseNotes(changelog, "0.1.1"), "Fixed updates.");
  assert.throws(() => releaseNotes(changelog, "0.2.0"), /exactly one/);
  assert.throws(() => releaseNotes(changelog, "0.1.1-beta.1"), /stable SemVer/);
});

function versionFixture(context) {
  const root = mkdtempSync(join(tmpdir(), "gdom-version-test-"));
  context.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "src-tauri"));
  writeFileSync(join(root, "package.json"), JSON.stringify({ version: "0.1.1" }));
  writeFileSync(join(root, "src-tauri/tauri.conf.json"), JSON.stringify({ version: "0.1.1" }));
  writeFileSync(join(root, "src-tauri/Cargo.toml"), `[package]
name = "gdom"
version = "0.1.1"
`);
  writeFileSync(join(root, "src-tauri/Cargo.lock"), `[[package]]
name = "gdom"
version = "0.1.1"
`);
  return root;
}

test("rejects stale lockfile and tag versions", (context) => {
  const root = versionFixture(context);
  assert.equal(checkVersions(root, "v0.1.1"), "0.1.1");
  assert.throws(() => checkVersions(root, "v0.1.2"), /mismatch/);
  writeFileSync(join(root, "src-tauri/Cargo.lock"), `[[package]]
name = "gdom"
version = "0.1.0"
`);
  assert.throws(() => checkVersions(root, "v0.1.1"), /versions must match/);
});

test("stages unsigned macOS DMG without requiring updater archive or signing secrets", (context) => {
  const root = versionFixture(context);
  const bundle = join(root, "src-tauri/target/aarch64-apple-darwin/release/bundle/dmg");
  mkdirSync(bundle, { recursive: true });
  writeFileSync(join(bundle, "GDOM.dmg"), "unsigned package");
  assert.doesNotThrow(() => stageArtifacts(root, "darwin-aarch64", join(root, "staged"), false));
});
