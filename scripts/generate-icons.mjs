import { execFileSync } from "node:child_process";
import { log } from "node:console";
import { copyFileSync, mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { argv, execPath } from "node:process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const source = join(root, "src/assets/gdom-icon.svg");
const destination = join(root, "src-tauri/icons");
const args = argv.slice(2);
if (args.length > 1 || (args.length === 1 && args[0] !== "--check")) {
  throw new Error("Usage: node scripts/generate-icons.mjs [--check]");
}
const check = args[0] === "--check";
const require = createRequire(import.meta.url);
const cliPackage = require.resolve("@tauri-apps/cli/package.json");
const { bin } = JSON.parse(readFileSync(cliPackage, "utf8"));
const cli = join(dirname(cliPackage), typeof bin === "string" ? bin : bin.tauri);
const output = mkdtempSync(join(tmpdir(), "gdom-icons-"));
const isDesktopIcon = (name) => /\.(png|ico|icns)$/.test(name);

try {
  // Invoke the lockfile's CLI with Node, not a platform-specific shell shim.
  execFileSync(execPath, [cli, "icon", source, "--output", output], {
    cwd: root,
    stdio: "inherit",
  });
  const generated = readdirSync(output, { withFileTypes: true })
    .filter((entry) => entry.isFile() && isDesktopIcon(entry.name))
    .map((entry) => entry.name);
  const existing = readdirSync(destination).filter(isDesktopIcon);
  if (generated.length === 0 || existing.some((name) => !generated.includes(name))) {
    throw new Error("Unexpected desktop icon set; review the Tauri CLI output before updating.");
  }
  for (const name of generated) {
    if (check) {
      if (!existing.includes(name) || !readFileSync(join(destination, name)).equals(readFileSync(join(output, name)))) {
        throw new Error(`Stale icon: ${name}. Run pnpm icons and commit the generated assets.`);
      }
    } else {
      copyFileSync(join(output, name), join(destination, name));
    }
  }
  log(`${check ? "Verified" : "Generated"} ${generated.length} desktop icons from gdom-icon.svg.`);
} finally {
  // Mobile outputs are not part of GDOM's desktop asset set.
  rmSync(output, { recursive: true, force: true });
}
