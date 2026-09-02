# valafmt

An opinionated, Prettier-style code formatter for [Vala](https://vala.dev).

There is no good Vala formatter: the community mostly relies on uncrustify,
which mishandles lambdas and can even drop code. valafmt is a dedicated
formatter with a full-fidelity Vala parser, built the way modern formatters
(Prettier, gofmt, rustfmt) are built.

## Style

The output style is modeled on **how Prettier formats TypeScript**, not on the
GNOME/elementary style guides:

- no space before call or declaration parentheses: `foo(bar)`, `void main()`
- braces on the same line: `if (x) {`
- line-width-driven wrapping: constructs stay on one line until they exceed
  the width, then break one element per line with a trailing comma where Vala
  allows one
- lambda arguments hug the call — the signature stays on the call line:

```vala
button.clicked.connect(() => {
    counter++;
});
```

- assignment right-hand sides that can break internally (calls, chains,
  ternaries, object creation) attach after the `=`; everything else breaks
  onto an indented continuation
- fluent method chains break before each `.`
- comments and blank lines are preserved (runs of blank lines collapse to one)

## Options

Two knobs, by design. There is no config file and there will be no brace-style,
spacing, or alignment options — that path leads to uncrustify.

```
valafmt [PATHS...]        format files/directories in place (*.vala, *.vapi)
  (no paths)              read stdin, write stdout
  --check                 don't write; list files that would change; exit 1 if any
  --stdout                print instead of writing in place
  --line-width <N>        preferred maximum line width (default 100)
  --indent <N>            indentation in spaces (default 4)
```

Exit codes: `0` ok · `1` `--check` found differences · `2` parse error ·
`3` internal error (input left untouched).

## Safety

valafmt refuses to touch code it cannot fully parse, and after every format it
re-lexes its own output and verifies the token stream — every token, every
comment, every preprocessor directive — is identical to the input (modulo
whitespace and trailing commas). If that check ever fails, valafmt reports an
internal error and leaves the file unchanged. The worst-case failure mode is a
no-op, never a corrupted file.

That guarantee is exercised continuously against a corpus of real Vala
projects (elementary Files/Terminal/Calendar/Code/Music/Photos, GNOME
Clocks/Geary/gitg/Shotwell): **99.7% of ~1500 files** format cleanly and
idempotently; the rest are refused with a clear error.

## Known limitation

`#if` / `#else` preprocessor branches are preserved verbatim, and files whose
branches each form valid syntax on their own format fine. A file where the
branches are *alternative fragments* (e.g. two competing method signatures
around an `#else`) does not concatenate into parseable code; valafmt refuses
such files rather than guessing. In the corpus this affects ~0.3% of files.

## Building

```
cargo build --release        # binary in target/release/valafmt
cargo test                   # snapshot + idempotency + safety tests
scripts/corpus.sh            # clone real Vala projects and sweep them
```
