// A toy language server, for `tests/lsp.spec.js`: the smallest scripted
// JSON-RPC responder that drives helix's real completion popup, hover and
// goto-definition end to end over the Web Worker transport (issue #144).
// Not part of the demo bundle — the spec loads this source into a Blob URL
// worker and hands it to `window.helixLanguageServers`, so the page never
// serves the file. Any language server that speaks LSP over `postMessage`
// with one JSON-RPC message per string is registered the same way.
//
// The wire format is what the transport documents: no `Content-Length`
// framing, each `postMessage` carries one complete message as a string.

const documents = new Map();

function respond(id, result) {
  postMessage(JSON.stringify({ jsonrpc: "2.0", id, result }));
}

function fail(id, code, message) {
  postMessage(JSON.stringify({ jsonrpc: "2.0", id, error: { code, message } }));
}

onmessage = (event) => {
  const message = JSON.parse(event.data);
  const { id, method, params } = message;
  switch (method) {
    case "initialize":
      respond(id, {
        capabilities: {
          // 1 = full document sync: didChange carries the whole text.
          textDocumentSync: 1,
          completionProvider: {},
          hoverProvider: true,
          definitionProvider: true,
        },
        serverInfo: { name: "toy" },
      });
      break;
    case "initialized":
      break;
    case "textDocument/didOpen":
      documents.set(params.textDocument.uri, params.textDocument.text);
      break;
    case "textDocument/didChange":
      documents.set(
        params.textDocument.uri,
        params.contentChanges.at(-1)?.text ?? "",
      );
      break;
    case "textDocument/didClose":
      documents.delete(params.textDocument.uri);
      break;
    case "textDocument/completion":
      respond(id, [
        {
          label: "toy_completion",
          // 1 = Text.
          kind: 1,
          detail: "from the toy server",
        },
      ]);
      break;
    case "textDocument/hover":
      respond(id, {
        contents: { kind: "markdown", value: "toy hover" },
      });
      break;
    case "textDocument/definition":
      // Always the same spot in the document that asked: line 2, column 4.
      respond(id, {
        uri: params.textDocument.uri,
        range: {
          start: { line: 2, character: 4 },
          end: { line: 2, character: 4 },
        },
      });
      break;
    case "shutdown":
      respond(id, null);
      break;
    case "exit":
      close();
      break;
    default:
      if (id !== undefined) {
        // -32601 = MethodNotFound.
        fail(id, -32601, `toy server does not implement ${method}`);
      }
  }
};
