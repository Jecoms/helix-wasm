# Third-party notices for the web bundle

The distributed wasm artifact statically links the C parser sources of the
tree-sitter grammars below (the `GRAMMARS` catalog in `build.rs`, fetched at
build time at the pinned revisions). All of them are MIT-licensed; the
copyright lines are reproduced verbatim from each repository's `LICENSE`
file at the pin, and the MIT license text follows at the end.

It also statically links the tree-sitter runtime those parsers run under: the
core C library plus the small subset of ICU headers that C includes, both
vendored in this repository under `stubs/tree-house-bindings/vendor/` and
compiled by that crate's `build.rs`. Their notices are in
"[tree-sitter runtime](#tree-sitter-runtime)" below.

It further embeds three kinds of runtime files from the in-tree helix port at
`../helix/runtime/`, read where they lie rather than copied into this crate:

- the tree-sitter query files under `helix/runtime/queries/`, for the
  languages the grammar catalog selects — see `queries/README.md`;
- the theme files under `helix/runtime/themes/`, for the curated set in
  `build.rs`'s `THEME_CATALOG` — see `themes/README.md`;
- the tutor text at `helix/runtime/tutor` — see `runtime/README.md`.

Those are helix's own files and MPL-2.0 like helix itself (see the
repository's top-level `LICENSE`) — with one exception: upstream carries
per-theme licenses in `helix/runtime/themes/licenses/` for themes
contributed under other terms, and one of the embedded themes has an entry
there. It is in "[Themes](#themes)" below.

When bumping a grammar pin, re-check its `LICENSE` and update the matching
entry here. When adding a theme to `THEME_CATALOG`, check
`helix/runtime/themes/licenses/` for a file matching its name and add a row
below if there is one. The same applies when re-vendoring
`stubs/tree-house-bindings` (its `Cargo.toml` header carries the refresh
recipe): re-check `vendor/LICENSE` and `vendor/src/unicode/LICENSE` against
the section below.

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

## Themes

The themes in `build.rs`'s `THEME_CATALOG` are helix's own files under
`helix/runtime/themes/`, MPL-2.0 like helix itself, except where upstream
records other terms in `helix/runtime/themes/licenses/`. One embedded theme
has such an entry:

| Theme | Upstream license file | License |
| --- | --- | --- |
| `everforest_dark.toml` | `helix/runtime/themes/licenses/everforest.LICENSE` | MIT — Copyright (c) 2019 sainnhe |

The MIT text is the one reproduced at the end of this file; the verbatim
file is at the path above.

## tree-sitter runtime

The parsers above are driven by the tree-sitter C runtime, which reaches the
artifact through the `tree-house-bindings` crate vendored at
`stubs/tree-house-bindings/` (the crate's own Rust code is MPL-2.0; see its
`LICENSE`). The C it compiles is two upstream trees:

| Component | Upstream | Pinned revision | License |
| --- | --- | --- | --- |
| tree-sitter core C library (`vendor/src`, `vendor/include`) | <https://github.com/tree-sitter/tree-sitter> | `v0.25.9` | MIT — Copyright (c) 2018-2024 Max Brunsfeld |
| ICU header subset (`vendor/src/unicode`: `utf8.h`, `utf16.h`, `umachine.h`, and the empty headers they reference) | <https://github.com/unicode-org/icu> | `552b01f61127d30d6589aa4bf99468224979b661` | Unicode license, reproduced below |

Paths are relative to `stubs/tree-house-bindings/`. The tree-sitter core is
under the same MIT license reproduced at the end of this file, with the
copyright line above; the verbatim file is `vendor/LICENSE`.

### COPYRIGHT AND PERMISSION NOTICE (ICU 58 and later)

Reproduced from `vendor/src/unicode/LICENSE`. Only the primary notice is
included here: the "Third-Party Software Licenses" section of that file
covers ICU data files (word-break dictionaries, the time zone database,
double-conversion) that this header-only subset does not contain. The
complete file ships in the repository at the path above.

```
Copyright © 1991-2019 Unicode, Inc. All rights reserved.
Distributed under the Terms of Use in https://www.unicode.org/copyright.html.

Permission is hereby granted, free of charge, to any person obtaining
a copy of the Unicode data files and any associated documentation
(the "Data Files") or Unicode software and any associated documentation
(the "Software") to deal in the Data Files or Software
without restriction, including without limitation the rights to use,
copy, modify, merge, publish, distribute, and/or sell copies of
the Data Files or Software, and to permit persons to whom the Data Files
or Software are furnished to do so, provided that either
(a) this copyright and permission notice appear with all copies
of the Data Files or Software, or
(b) this copyright and permission notice appear in associated
Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT OF THIRD PARTY RIGHTS.
IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS
NOTICE BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL
DAMAGES, OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE,
DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR
PERFORMANCE OF THE DATA FILES OR SOFTWARE.

Except as contained in this notice, the name of a copyright holder
shall not be used in advertising or otherwise to promote the sale,
use or other dealings in these Data Files or Software without prior
written authorization of the copyright holder.
```

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
