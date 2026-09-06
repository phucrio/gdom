import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

const iconFile = (name: string) => readFileSync(new URL(`../../src-tauri/icons/${name}`, import.meta.url));
const pngSizes = [
  ["32x32.png", 32],
  ["64x64.png", 64],
  ["128x128.png", 128],
  ["128x128@2x.png", 256],
  ["icon.png", 512],
  ["Square30x30Logo.png", 30],
  ["Square44x44Logo.png", 44],
  ["Square71x71Logo.png", 71],
  ["Square89x89Logo.png", 89],
  ["Square107x107Logo.png", 107],
  ["Square142x142Logo.png", 142],
  ["Square150x150Logo.png", 150],
  ["Square284x284Logo.png", 284],
  ["Square310x310Logo.png", 310],
  ["StoreLogo.png", 50],
] as const;

describe("GDOM application icons", () => {
  it("keeps the master SVG self-contained", () => {
    const source = readFileSync(new URL("./gdom-icon.svg", import.meta.url), "utf8");
    expect(source).toContain('viewBox="0 0 512 512"');
    expect(source).toContain("<title>GDOM</title>");
    expect(source).not.toMatch(/<(?:script|image|foreignObject)\b/i);
    expect(source).not.toMatch(/(?:href|src)=["'](?:https?:|data:|\/\/)/i);
  });

  it.each(pngSizes)("%s has the required square RGBA8 dimensions", (name, size) => {
    const png = iconFile(name);
    expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
    expect(png.toString("ascii", 12, 16)).toBe("IHDR");
    expect(png.readUInt32BE(16)).toBe(size);
    expect(png.readUInt32BE(20)).toBe(size);
    expect(png[24]).toBe(8);
    expect(png[25]).toBe(6);
  });

  it("bundles every configured icon", () => {
    const config = JSON.parse(readFileSync(new URL("../../src-tauri/tauri.conf.json", import.meta.url), "utf8")) as {
      bundle: { icon: string[] };
    };
    for (const path of config.bundle.icon) {
      expect(readFileSync(new URL(`../../src-tauri/${path}`, import.meta.url)).length).toBeGreaterThan(0);
    }
  });

  it("includes the Windows resolutions with the 32px development layer first", () => {
    const ico = iconFile("icon.ico");
    expect(ico.readUInt16LE(0)).toBe(0);
    expect(ico.readUInt16LE(2)).toBe(1);
    const count = ico.readUInt16LE(4);
    expect(count).toBeGreaterThanOrEqual(6);
    expect(ico[6]).toBe(32);
    const sizes: number[] = [];
    for (let index = 0; index < count; index += 1) {
      const entry = 6 + index * 16;
      const width = ico[entry] || 256;
      expect(ico[entry + 1] || 256).toBe(width);
      const length = ico.readUInt32LE(entry + 8);
      const offset = ico.readUInt32LE(entry + 12);
      expect(length).toBeGreaterThan(0);
      expect(offset).toBeGreaterThanOrEqual(6 + count * 16);
      expect(offset + length).toBeLessThanOrEqual(ico.length);
      sizes.push(width);
    }
    expect(sizes).toEqual(expect.arrayContaining([16, 24, 32, 48, 64, 256]));
  });

  it("keeps the macOS ICNS container complete", () => {
    const icns = iconFile("icon.icns");
    expect(icns.toString("ascii", 0, 4)).toBe("icns");
    expect(icns.readUInt32BE(4)).toBe(icns.length);
    let offset = 8;
    const kinds: string[] = [];
    while (offset < icns.length) {
      expect(offset + 8).toBeLessThanOrEqual(icns.length);
      const length = icns.readUInt32BE(offset + 4);
      expect(length).toBeGreaterThan(8);
      expect(offset + length).toBeLessThanOrEqual(icns.length);
      kinds.push(icns.toString("ascii", offset, offset + 4));
      offset += length;
    }
    expect(offset).toBe(icns.length);
    expect(kinds.length).toBeGreaterThan(1);
    expect(kinds).toEqual([...kinds].sort());
  });
});
