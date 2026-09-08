import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { argv, env } from "node:process";
import { pathToFileURL } from "node:url";

export const platforms = {
  "windows-x86_64": { target: "x86_64-pc-windows-msvc", suffix: ".exe" },
  "windows-aarch64": { target: "aarch64-pc-windows-msvc", suffix: ".exe" },
  "darwin-x86_64": { target: "x86_64-apple-darwin", suffix: ".app.tar.gz" },
  "darwin-aarch64": { target: "aarch64-apple-darwin", suffix: ".app.tar.gz" },
  "linux-x86_64": { target: "x86_64-unknown-linux-gnu", suffix: ".AppImage" },
  "linux-aarch64": { target: "aarch64-unknown-linux-gnu", suffix: ".AppImage" },
};
const read = (path) => readFileSync(path, "utf8");
const git = (...args) => execFileSync("git", args, { encoding: "utf8" }).trim();
const digest = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");
const stableVersion = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

export function releaseNotes(changelog, version) {
  if (!stableVersion.test(version)) throw new Error("Release version must be stable SemVer.");
  const sections = changelog.split(/^## /m);
  const matching = sections.filter((section) => section.startsWith(`[${version}] - `));
  if (matching.length !== 1) throw new Error("Expected exactly one dated changelog section for the release.");
  const [heading, ...body] = matching[0].split("\n");
  if (!/^\[\d+\.\d+\.\d+\] - \d{4}-\d{2}-\d{2}$/.test(heading.trim()) || !body.join("\n").trim()) {
    throw new Error("Release changelog needs a date and nonempty release notes.");
  }
  return body.join("\n").trim();
}

export function checkVersions(root, tag) {
  const version = JSON.parse(read(join(root, "package.json"))).version;
  if (!stableVersion.test(version) || (tag && tag !== `v${version}`)) throw new Error("Tag/package version mismatch or unstable version.");
  const configuration = JSON.parse(read(join(root, "src-tauri/tauri.conf.json")));
  const manifest = read(join(root, "src-tauri/Cargo.toml")).split("[package]")[1]?.split(/^\[/m)[0];
  const lockPackage = read(join(root, "src-tauri/Cargo.lock")).split("[[package]]").find((entry) => /^name = "gdom"$/m.test(entry));
  if (configuration.version !== version || !manifest?.includes(`version = "${version}"`) || !lockPackage?.includes(`version = "${version}"`)) {
    throw new Error("Application versions must match in package, Tauri, Cargo and Cargo.lock.");
  }
  return version;
}

function filesUnder(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesUnder(path) : [path];
  });
}

export function stageArtifacts(root, platform, output, signed) {
  const definition = platforms[platform];
  if (!definition) throw new Error("Unknown release platform.");
  const version = checkVersions(root);
  const candidates = filesUnder(join(root, "src-tauri/target", definition.target, "release/bundle"));
  const suffixes = platform.startsWith("darwin-") ? [...(signed ? [definition.suffix] : []), ".dmg"] : [definition.suffix];
  mkdirSync(output, { recursive: true });
  const artifacts = [];
  for (const suffix of suffixes) {
    const matches = candidates.filter((path) => path.endsWith(suffix));
    if (matches.length !== 1) throw new Error(`Expected exactly one ${suffix} artifact for ${platform}.`);
    const filename = `GDOM_${version}_${platform}${suffix}`;
    copyFileSync(matches[0], join(output, filename));
    artifacts.push({ filename, sha256: digest(matches[0]) });
    if (signed && suffix === definition.suffix) {
      execFileSync("cargo", ["run", "--locked", "--manifest-path", join(root, "src-tauri/Cargo.toml"), "--example", "verify-update-artifact", "--target", definition.target, "--", matches[0], `${matches[0]}.sig`], { stdio: "inherit" });
      const signature = read(`${matches[0]}.sig`).trim();
      if (!signature) throw new Error("Missing updater signature.");
      writeFileSync(join(output, `${filename}.sig`), signature);
    }
  }
  const receipt = { platform, version, commit: git("rev-parse", "HEAD"), signed, artifacts };
  writeFileSync(join(output, "receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);
}

export function aggregateArtifacts(input, output, version, commit, repository, notes) {
  if (!stableVersion.test(version) || !/^[\w.-]+\/[\w.-]+$/.test(repository)) throw new Error("Invalid release identity.");
  const directories = readdirSync(input, { withFileTypes: true }).filter((entry) => entry.isDirectory());
  if (directories.length !== Object.keys(platforms).length) throw new Error("Release requires exactly six platform bundles.");
  const manifest = { version, notes, pub_date: new Date().toISOString(), platforms: {} };
  mkdirSync(output, { recursive: true });
  for (const directory of directories) {
    const source = join(input, directory.name);
    const receipt = JSON.parse(read(join(source, "receipt.json")));
    const definition = platforms[receipt.platform];
    if (!definition || manifest.platforms[receipt.platform] || receipt.version !== version || receipt.commit !== commit || receipt.signed !== true) {
      throw new Error("Mismatched, duplicate or unsigned release receipt.");
    }
    const expectedSuffixes = [definition.suffix, ...(receipt.platform.startsWith("darwin-") ? [".dmg"] : [])];
    const expectedNames = expectedSuffixes.map((suffix) => `GDOM_${version}_${receipt.platform}${suffix}`);
    if (receipt.artifacts.length !== expectedNames.length || new Set(receipt.artifacts.map((artifact) => artifact.filename)).size !== expectedNames.length) throw new Error("Incomplete artifact receipt.");
    for (const artifact of receipt.artifacts) {
      if (!expectedNames.includes(artifact.filename) || digest(join(source, artifact.filename)) !== artifact.sha256) throw new Error("Artifact identity or checksum mismatch.");
      copyFileSync(join(source, artifact.filename), join(output, artifact.filename));
    }
    const filename = expectedNames[0];
    const signature = read(join(source, `${filename}.sig`)).trim();
    if (!signature) throw new Error("Missing updater signature.");
    copyFileSync(join(source, `${filename}.sig`), join(output, `${filename}.sig`));
    manifest.platforms[receipt.platform] = { signature, url: `https://github.com/${repository}/releases/download/v${version}/${filename}` };
  }
  writeFileSync(join(output, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  writeFileSync(join(output, "release-notes.md"), `${notes}\n`);
  return manifest;
}

if (argv[1] && import.meta.url === pathToFileURL(resolve(argv[1])).href) {
  const [command, ...args] = argv.slice(2);
  switch (command) {
    case "check": checkVersions("."); break;
    case "preflight": {
      const tag = args[0];
      const version = checkVersions(".", tag);
      if (!tag) throw new Error("A release tag is required.");
      if (git("rev-parse", `${tag}^{commit}`) !== git("rev-parse", "HEAD")) throw new Error("Tag does not resolve to checkout SHA.");
      git("merge-base", "--is-ancestor", "HEAD", "origin/main");
      releaseNotes(read("CHANGELOG.md"), version);
      break;
    }
    case "stage": stageArtifacts(".", args[0], args[1], args[2] === "signed"); break;
    case "aggregate": {
      const version = checkVersions(".", env.GITHUB_REF_NAME);
      aggregateArtifacts(args[0], args[1], version, git("rev-parse", "HEAD"), env.GITHUB_REPOSITORY, releaseNotes(read("CHANGELOG.md"), version));
      break;
    }
    default: throw new Error("Usage: release.mjs check | preflight TAG | stage PLATFORM OUTPUT [signed] | aggregate INPUT OUTPUT");
  }
}
