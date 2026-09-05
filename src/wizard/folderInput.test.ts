import { describe, expect, it } from "vitest";

import { parseFolderInput } from "./folderInput.ts";

const FOLDER_ID = "1AbCDefGhijkLMNOPqrstuvWxyz01234";

describe("parseFolderInput", () => {
  it("parses representative Drive folder URLs", () => {
    expect(parseFolderInput(`https://drive.google.com/drive/folders/${FOLDER_ID}`)).toEqual({
      ok: true,
      folderId: FOLDER_ID,
      source: "url",
    });
    expect(parseFolderInput(`https://drive.google.com/drive/u/0/folders/${FOLDER_ID}`)).toEqual({
      ok: true,
      folderId: FOLDER_ID,
      source: "url",
    });
    expect(parseFolderInput(`https://drive.google.com/open?id=${FOLDER_ID}`)).toEqual({
      ok: true,
      folderId: FOLDER_ID,
      source: "url",
    });
    expect(
      parseFolderInput(`https://drive.google.com/drive/folders/${FOLDER_ID}?usp=sharing`),
    ).toEqual({
      ok: true,
      folderId: FOLDER_ID,
      source: "url",
    });
  });

  it("accepts a raw Drive folder ID", () => {
    expect(parseFolderInput(`  ${FOLDER_ID}  `)).toEqual({
      ok: true,
      folderId: FOLDER_ID,
      source: "id",
    });
  });

  it("fails live validation for empty or invalid input", () => {
    expect(parseFolderInput("")).toEqual({ ok: false, reason: "empty" });
    expect(parseFolderInput("   ")).toEqual({ ok: false, reason: "empty" });
    expect(parseFolderInput("https://example.com/folders/xyz")).toEqual({
      ok: false,
      reason: "invalid",
    });
    expect(parseFolderInput("not a folder")).toEqual({ ok: false, reason: "invalid" });
    expect(parseFolderInput("https://drive.google.com/file/d/1AbCDefGhijkLMNOPqrstuvWxyz01234")).toEqual({
      ok: false,
      reason: "invalid",
    });
    expect(parseFolderInput("root")).toEqual({ ok: false, reason: "invalid" });
    expect(parseFolderInput("https://drive.google.com/drive/folders/root")).toEqual({
      ok: false,
      reason: "invalid",
    });
  });

  it("accepts scheme-less and www Drive folder URLs", () => {
    expect(parseFolderInput(`drive.google.com/drive/folders/${FOLDER_ID}`)).toEqual({
      ok: true,
      folderId: FOLDER_ID,
      source: "url",
    });
    expect(parseFolderInput(`https://www.drive.google.com/drive/folders/${FOLDER_ID}`)).toEqual({
      ok: true,
      folderId: FOLDER_ID,
      source: "url",
    });
  });
});
