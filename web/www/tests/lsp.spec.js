// Language servers over a host-supplied Web Worker transport (issue #144).
// The browser cannot spawn a server process, so a page hands helix a
// `Worker` per server name (`window.helixLanguageServers`) alongside the
// `languages.toml` that declares it (`window.helixLanguages`); helix's
// unmodified LSP client then runs over `postMessage`. The server on the
// other end here is `../toy-lsp-worker.js`, a scripted responder, and the
// three features asserted are the ones that cross the most machinery: the
// completion popup (an async hook, debounced on a browser timer, fanning
// its request out through the task shim), hover (a request/response round
// trip rendered in a popup) and `gd` (a response that moves the cursor).
import { readFileSync } from "node:fs";
import { test, expect } from "@playwright/test";
import { bootEditor, getState, getText, terminalText } from "./helpers.js";

const WORKER = readFileSync(
  new URL("../toy-lsp-worker.js", import.meta.url),
  "utf8",
);

// The page's `languages.toml`: one server, one language that uses it. The
// `command` is what native helix would spawn; here the registered name is
// the whole of the match and the command is ignored.
const LANGUAGES = `
[language-server.toy]
command = "toy-lsp"

[[language]]
name = "toy"
scope = "source.toy"
file-types = ["toy"]
roots = []
language-servers = ["toy"]
`;

// Three lines, so the toy server's fixed definition target (line 2,
// column 4) is inside the document.
const DOCUMENT = "fn one\nfn two\n    target\n";
const PATH = "/demo.toy";

// Boot with the toy server registered, then open a document of its
// language — which is what makes helix launch the server. The worker is
// built from the source above at page-init time: a Blob URL keeps the
// fixture out of the served bundle.
async function bootWithToyServer(page) {
  await page.addInitScript(
    ({ languages, worker }) => {
      window.helixLanguages = languages;
      const url = URL.createObjectURL(
        new Blob([worker], { type: "text/javascript" }),
      );
      window.helixLanguageServers = { toy: new Worker(url) };
    },
    { languages: LANGUAGES, worker: WORKER },
  );
  await bootEditor(page);
  await page.evaluate(
    ([path, text]) => window.helixVfs.write(path, text),
    [PATH, DOCUMENT],
  );
  await page.keyboard.type(`:o ${PATH}`);
  await page.keyboard.press("Enter");
  await expect.poll(async () => (await getState(page)).path).toBe(PATH);
}

test("the completion popup is fed by the worker server", async ({ page }) => {
  await bootWithToyServer(page);

  // Auto-completion: two word characters typed in insert mode start the
  // debounced request; the popup lists what the server answered.
  await page.keyboard.press("i");
  await page.keyboard.type("to");
  await expect.poll(() => terminalText(page)).toContain("toy_completion");

  // Accepting the item is the proof the popup is helix's real one, wired
  // to the buffer: Tab selects the first entry (the popup opens with none
  // selected), Enter commits it, and the typed prefix is replaced by the
  // label in front of what the document held.
  await page.keyboard.press("Tab");
  await page.keyboard.press("Enter");
  await expect.poll(() => getText(page)).toBe(`toy_completion${DOCUMENT}`);
});

test("hover shows the server's answer in a popup", async ({ page }) => {
  await bootWithToyServer(page);

  await page.keyboard.press(" ");
  await page.keyboard.press("k");
  await expect.poll(() => terminalText(page)).toContain("toy hover");
});

test("gd jumps to the definition the server returns", async ({ page }) => {
  await bootWithToyServer(page);

  expect((await getState(page)).cursor).toEqual({ row: 0, col: 0 });
  await page.keyboard.press("g");
  await page.keyboard.press("d");
  await expect
    .poll(async () => (await getState(page)).cursor)
    .toEqual({ row: 2, col: 4 });
});

test(":lsp-restart reconnects to the same worker", async ({ page }) => {
  await bootWithToyServer(page);
  await page.keyboard.press(" ");
  await page.keyboard.press("k");
  await expect.poll(() => terminalText(page)).toContain("toy hover");
  await page.keyboard.press("Escape");
  await expect.poll(() => terminalText(page)).not.toContain("toy hover");

  // The restart connects afresh to the port the page registered. Helix
  // shuts the old client down *after* the new one has attached, so the old
  // client's `exit` must not reach the worker — the toy server honors it
  // with `close()`, which would leave the new connection talking to
  // nothing. The proof is a second round trip after the restart. The new
  // client only supports hover once the server has answered its
  // `initialize`, and nothing on the surface says when that is: a hover
  // asked for before then reports "No configured language server supports
  // hover", so ask, read which of the two answers came, and ask again on
  // that one.
  await page.keyboard.type(":lsp-restart");
  await page.keyboard.press("Enter");
  const NO_SERVER = "No configured language server supports hover";
  for (;;) {
    await page.keyboard.press(" ");
    await page.keyboard.press("k");
    let outcome;
    await expect
      .poll(async () => {
        const text = await terminalText(page);
        if (text.includes("toy hover")) outcome = "hover";
        else if (text.includes(NO_SERVER)) outcome = "not yet";
        return outcome;
      })
      .toBeDefined();
    if (outcome === "hover") break;
    // The next key clears the statusline; wait for that so the read above
    // cannot see this attempt's message again.
    await page.keyboard.press("Escape");
    await expect.poll(() => terminalText(page)).not.toContain(NO_SERVER);
  }
});

test("a malformed message from the server neither traps nor wedges", async ({
  page,
}) => {
  await bootWithToyServer(page);
  await page.keyboard.press(" ");
  await page.keyboard.press("k");
  await expect.poll(() => terminalText(page)).toContain("toy hover");
  await page.keyboard.press("Escape");

  // The first thing a hand-written server produces is a string the
  // transport cannot parse. That is its error branch: it fails the pending
  // requests, injects `exit`, and helix reports the server gone — the
  // editor itself keeps taking input. The page asks the toy worker to
  // misbehave through the same port helix talks on; the worker answers
  // helix, not the page.
  await page.evaluate(() =>
    window.helixLanguageServers.toy.postMessage(
      JSON.stringify({ jsonrpc: "2.0", method: "toy/emitGarbage" }),
    ),
  );
  await expect.poll(() => terminalText(page)).toContain("Language server exited");
  await page.keyboard.press("i");
  await page.keyboard.type("ok ");
  await expect.poll(() => getText(page)).toBe(`ok ${DOCUMENT}`);
});

test("a server name with no worker fails the way it always has", async ({
  page,
}) => {
  // The language config alone, no `helixLanguageServers`: helix asks for a
  // server nothing was registered under, and the document opens without
  // one — the pre-#144 behavior, with hover's own message as the evidence.
  await page.addInitScript((languages) => {
    window.helixLanguages = languages;
  }, LANGUAGES);
  await bootEditor(page);
  await page.keyboard.type(`:o ${PATH}`);
  await page.keyboard.press("Enter");
  await expect.poll(async () => (await getState(page)).path).toBe(PATH);

  await page.keyboard.press(" ");
  await page.keyboard.press("k");
  await expect
    .poll(() => terminalText(page))
    .toContain("No configured language server supports hover");
});
