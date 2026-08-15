# Third-party notices for the web bundle

The distributed wasm artifact statically links the C parser sources of the
tree-sitter grammars below (the `GRAMMARS` catalog in `build.rs`, fetched at
build time at the pinned revisions). All of them are MIT-licensed; the
copyright lines are reproduced verbatim from each repository's `LICENSE`
file at the pin, and the MIT license text follows at the end.

The vendored query files under `queries/` are copies of helix's own
`runtime/queries/` (MPL-2.0, like helix itself; see the repository's
top-level `LICENSE`), documented in `queries/README.md`.

When bumping a grammar pin, re-check its `LICENSE` and update the matching
entry here.

## Grammars

| Grammar | Repository | Pinned revision | Copyright |
| --- | --- | --- | --- |
| c | <https://github.com/tree-sitter/tree-sitter-c> | `7175a6dd5fc1cee660dce6fe23f6043d75af424a` | Copyright (c) 2014 Max Brunsfeld |
| go | <https://github.com/tree-sitter/tree-sitter-go> | `64457ea6b73ef5422ed1687178d4545c3e91334a` | Copyright (c) 2014 Max Brunsfeld |
| java | <https://github.com/tree-sitter/tree-sitter-java> | `09d650def6cdf7f479f4b78f595e9ef5b58ce31e` | Copyright (c) 2017 Ayman Nadeem |
| javascript | <https://github.com/tree-sitter/tree-sitter-javascript> | `f772967f7b7bc7c28f845be2420a38472b16a8ee` | Copyright (c) 2014 Max Brunsfeld |
| python | <https://github.com/tree-sitter/tree-sitter-python> | `4bfdd9033a2225cc95032ce77066b7aeca9e2efc` | Copyright (c) 2016 Max Brunsfeld |
| regex | <https://github.com/tree-sitter/tree-sitter-regex> | `e1cfca3c79896ff79842f057ea13e529b66af636` | Copyright (c) 2014 Max Brunsfeld |
| rust | <https://github.com/tree-sitter/tree-sitter-rust> | `1f63b33efee17e833e0ea29266dd3d713e27e321` | Copyright (c) 2017 Maxim Sokolov |
| toml | <https://github.com/ikatyang/tree-sitter-toml> | `7cff70bbcbbc62001b465603ca1ea88edd668704` | Copyright (c) Ika \<ikatyang@gmail.com\> (<https://github.com/ikatyang>) |

## The MIT License (MIT)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to
deal in the Software without restriction, including without limitation the
rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
