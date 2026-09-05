function srgbChannel(byte: number): number {
  const sample = byte / 255;
  if (sample <= 0.04045) {
    return sample / 12.92;
  }
  return ((sample + 0.055) / 1.055) ** 2.4;
}

export function relativeLuminance(hex: string): number {
  const normalized = expandHex(hex);
  const red = Number.parseInt(normalized.slice(1, 3), 16);
  const green = Number.parseInt(normalized.slice(3, 5), 16);
  const blue = Number.parseInt(normalized.slice(5, 7), 16);
  return 0.2126 * srgbChannel(red) + 0.7152 * srgbChannel(green) + 0.0722 * srgbChannel(blue);
}

export function contrastRatio(first: string, second: string): number {
  const left = relativeLuminance(first);
  const right = relativeLuminance(second);
  const lighter = Math.max(left, right);
  const darker = Math.min(left, right);
  return (lighter + 0.05) / (darker + 0.05);
}

export function expandHex(hex: string): string {
  const trimmed = hex.trim().toLowerCase();
  const short = /^#([0-9a-f])([0-9a-f])([0-9a-f])$/.exec(trimmed);
  if (short) {
    return `#${short[1]}${short[1]}${short[2]}${short[2]}${short[3]}${short[3]}`;
  }
  const long = /^#([0-9a-f]{6})$/.exec(trimmed);
  if (long) {
    return `#${long[1]}`;
  }
  throw new Error(`unsupported color: ${hex}`);
}

export function parseRootColorTokens(css: string): Record<string, string> {
  const root = css.match(/:root\s*\{([\s\S]*?)\}/);
  if (root === null || root[1] === undefined) {
    throw new Error("missing :root color tokens");
  }

  const tokens: Record<string, string> = {};
  const declaration = /--([a-z0-9-]+)\s*:\s*(#[0-9a-f]{3,8})/gi;
  let match = declaration.exec(root[1]);
  while (match !== null) {
    const name = match[1];
    const value = match[2];
    if (name !== undefined && value !== undefined) {
      tokens[name] = expandHex(value);
    }
    match = declaration.exec(root[1]);
  }
  return tokens;
}

export function parseRuleBlock(css: string, selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const block = css.match(new RegExp(`${escaped}\\s*\\{([\\s\\S]*?)\\}`));
  if (block === null || block[1] === undefined) {
    throw new Error(`missing CSS rule for ${selector}`);
  }
  return block[1];
}

export function declarationValue(block: string, property: string): string {
  const match = block.match(new RegExp(`${property}\\s*:\\s*([^;]+);`));
  if (match === null || match[1] === undefined) {
    throw new Error(`missing ${property} in rule`);
  }
  return match[1].trim();
}

export function resolveCssColor(value: string, tokens: Record<string, string>): string {
  const variable = value.match(/var\(--([a-z0-9-]+)\)/i);
  if (variable?.[1] !== undefined) {
    const resolved = tokens[variable[1]];
    if (resolved === undefined) {
      throw new Error(`unresolved token --${variable[1]}`);
    }
    return resolved;
  }

  const hex = value.match(/#[0-9a-f]{3,8}/i);
  if (hex?.[0] !== undefined) {
    return expandHex(hex[0]);
  }

  throw new Error(`no hex color in ${value}`);
}
