// `:download-all` — the whole store out of the page as one zip (issue
// #110), where `:download` gets one file (issue #67). What these assert on
// is the archive itself: a real browser download, and the bytes in it read
// back the way an extractor reads them, from the central directory
// outwards. Parsing it here rather than adding a JS unzip dependency keeps
// the assertion one about *format compliance* — a zip only this suite can
// open would pass a round-trip test and still be useless to a reader.
import { test, expect } from "@playwright/test";
import { bootEditor, getText, terminalText, vfsRead } from "./helpers.js";

// Type a command line and run it.
async function run(page, line) {
  await page.keyboard.type(line);
  await page.keyboard.press("Enter");
}

// Put `text` in the buffer, from normal mode, and come back to normal mode.
async function typeText(page, text) {
  await page.keyboard.press("i");
  await page.keyboard.type(text);
  await page.keyboard.press("Escape");
  await expect.poll(() => getText(page)).toContain(text);
}

// Write the current buffer to `path` and wait for the save to land.
async function saveAs(page, path) {
  await run(page, `:w ${path}`);
  await expect.poll(() => vfsRead(page, path)).not.toBeUndefined();
}

// CRC-32/ISO-HDLC, the check a zip entry carries. Computed here rather than
// trusted so the archive is verified against something outside itself.
function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

// Read `buffer` as a zip the way an extractor does: find the end-of-
// central-directory record, walk the directory it points at, and follow
// each entry to its local header. Returns a `Map` of name -> text, and
// throws if anything about the archive does not add up — a wrong CRC, a
// compressed entry (nothing here writes one), or a directory record whose
// offset does not land on a local header for the same name.
function unzip(buffer) {
  // The end record is the last 22 bytes exactly: it is variable-length only
  // when the archive carries a comment, and this one never does.
  const end = buffer.length - 22;
  expect(buffer.readUInt32LE(end), "end-of-central-directory signature").toBe(
    0x06054b50,
  );
  const count = buffer.readUInt16LE(end + 10);
  let offset = buffer.readUInt32LE(end + 16);
  const files = new Map();
  for (let i = 0; i < count; i += 1) {
    expect(buffer.readUInt32LE(offset), "central directory signature").toBe(
      0x02014b50,
    );
    expect(buffer.readUInt16LE(offset + 10), "compression method").toBe(0);
    const crc = buffer.readUInt32LE(offset + 16);
    const size = buffer.readUInt32LE(offset + 24);
    const nameLength = buffer.readUInt16LE(offset + 28);
    const extraLength = buffer.readUInt16LE(offset + 30);
    const commentLength = buffer.readUInt16LE(offset + 32);
    const local = buffer.readUInt32LE(offset + 42);
    const name = buffer.toString("utf8", offset + 46, offset + 46 + nameLength);

    expect(buffer.readUInt32LE(local), `local header for ${name}`).toBe(
      0x04034b50,
    );
    const localName = buffer.readUInt16LE(local + 26);
    const localExtra = buffer.readUInt16LE(local + 28);
    expect(buffer.toString("utf8", local + 30, local + 30 + localName)).toBe(
      name,
    );
    const start = local + 30 + localName + localExtra;
    const bytes = buffer.subarray(start, start + size);
    expect(crc32(bytes), `CRC of ${name}`).toBe(crc);

    files.set(name, bytes.toString("utf8"));
    offset += 46 + nameLength + extraLength + commentLength;
  }
  expect(offset, "the directory ends where the end record begins").toBe(end);
  return files;
}

// Run `line` and return the archive it downloads, as
// `{ name, files }` with `files` a `Map` of member name -> text.
async function archiveFrom(page, line) {
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    run(page, line),
  ]);
  const stream = await download.createReadStream();
  const chunks = [];
  for await (const chunk of stream) {
    chunks.push(chunk);
  }
  return {
    name: download.suggestedFilename(),
    files: unzip(Buffer.concat(chunks)),
  };
}

test(":download-all packs every file the session saved", async ({ page }) => {
  await bootEditor(page);

  await typeText(page, "first file");
  await saveAs(page, "/notes.txt");
  await run(page, ":new");
  await typeText(page, "second file");
  await saveAs(page, "/proj/second.txt");

  const archive = await archiveFrom(page, ":download-all");
  expect(archive.name).toBe("helix-session.zip");
  // Store keys are absolute; archive members are relative, because an
  // absolute member is what makes an extractor refuse an archive. A key
  // with directories in it keeps them — an archive has somewhere to put
  // them, which is the whole difference from a single download.
  expect([...archive.files.keys()]).toEqual(["notes.txt", "proj/second.txt"]);
  expect(archive.files.get("notes.txt")).toBe("first file\n");
  expect(archive.files.get("proj/second.txt")).toBe("second file\n");
});

test(":download-all leaves out what boot seeded, edited or not", async ({
  page,
}) => {
  await bootEditor(page);

  // The store holds far more than the reader's work: the bundled themes and
  // the tutor text under the runtime directory, and the sample files. None
  // of that is anyone's session.
  const seeded = await page.evaluate(() => window.helixVfs.list());
  expect(seeded).toContain("/welcome.txt");
  expect(seeded.some((path) => path.startsWith("/.config/helix/runtime/"))).toBe(
    true,
  );

  await typeText(page, "mine");
  await saveAs(page, "/mine.txt");

  let archive = await archiveFrom(page, ":download-all");
  expect([...archive.files.keys()]).toEqual(["mine.txt"]);

  // The mark is on the key and it is permanent: editing a seeded file and
  // saving it does not put it in the archive. Deliberate (one changed color
  // would otherwise drag a whole copied theme into every export), and the
  // trap it would be if unsaid is what the README and the command's own
  // `doc` are for.
  await run(page, ":o /welcome.txt");
  await expect.poll(() => getText(page)).toContain("Welcome to the browser");
  await page.keyboard.press("i");
  await page.keyboard.type("edited. ");
  await page.keyboard.press("Escape");
  await run(page, ":w");
  await expect.poll(() => vfsRead(page, "/welcome.txt")).toContain("edited. ");

  archive = await archiveFrom(page, ":download-all");
  expect([...archive.files.keys()]).toEqual(["mine.txt"]);

  // Saving it under a name boot never seeded is the way out, and the one
  // the docs point at.
  await run(page, ":w /welcome-edited.txt");
  await expect
    .poll(() => vfsRead(page, "/welcome-edited.txt"))
    .toContain("edited. ");

  archive = await archiveFrom(page, ":download-all");
  expect([...archive.files.keys()]).toEqual([
    "mine.txt",
    "welcome-edited.txt",
  ]);
  expect(archive.files.get("welcome-edited.txt")).toContain("edited. ");
});

test(":download-all refuses over an unsaved buffer; :download-all! does not", async ({
  page,
}) => {
  await bootEditor(page);

  await typeText(page, "saved");
  await saveAs(page, "/draft.txt");

  // Edit past the save. The archive is the *store*, so it would disagree
  // with the screen — which is the one thing this command must not do
  // quietly, being the command you reach for when the tab is about to die.
  await page.keyboard.press("A");
  await page.keyboard.type(" and then some");
  await page.keyboard.press("Escape");

  let downloads = 0;
  page.on("download", () => {
    downloads += 1;
  });

  await run(page, ":download-all");
  await expect.poll(() => terminalText(page)).toContain("Unsaved buffers");
  expect(downloads).toBe(0);

  // The bang means "as it stands": the archive is built, and what it holds
  // for that buffer is the last saved copy, not the edit.
  const archive = await archiveFrom(page, ":download-all!");
  expect(archive.files.get("draft.txt")).toBe("saved\n");
  expect(await getText(page)).toContain("and then some");
});

test(":download-all names the archive from its argument", async ({ page }) => {
  await bootEditor(page);

  await typeText(page, "work");
  await saveAs(page, "/work.txt");

  // A download has a name, not a path: directories are a store key's
  // business, the same rule `:download` follows.
  const archive = await archiveFrom(page, ":download-all /tmp/backup.zip");
  expect(archive.name).toBe("backup.zip");
  expect([...archive.files.keys()]).toEqual(["work.txt"]);
});

test(":download-all refuses rather than handing over an empty archive", async ({
  page,
}) => {
  await bootEditor(page);

  let downloads = 0;
  page.on("download", () => {
    downloads += 1;
  });

  // Boot has seeded plenty and the reader has written nothing, so there is
  // an archive to build and no reason to build it. An empty zip is a valid
  // zip, which is exactly why handing one over would read as success.
  await run(page, ":download-all");
  await expect.poll(() => terminalText(page)).toContain("Nothing to export");
  expect(downloads).toBe(0);
});
