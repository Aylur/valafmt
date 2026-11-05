# valafmt

An opinionated code formatter for [Vala](https://vala.dev).

> [!NOTE]
>
> _This project is vibe coded_. I feel like formatting is a boring,
> well-understood problem that has been solved numerous times and it is
> completely fine to let an LLM generate another one. I was not happy with
> uncrustify or elementary's formatter, I just wanted the
> [Prettier](https://prettier.io/) experience for Vala, but did not have the
> will or the time to write it myself.

## Known limitation

`#if` / `#else` preprocessor branches are preserved verbatim, and files whose
branches each form valid syntax on their own format fine. A file where the
branches are _alternative fragments_ (e.g. two competing method signatures
around an `#else`) does not concatenate into parseable code; valafmt refuses
such files rather than guessing. In the corpus this affects ~0.3% of files.

## Building

```sh
cargo build --release # binary in target/release/valafmt
```

## Ignoring code

```vala
// valafmt-ignore
var mx = matrix(
  1, 0, 0,
  0, 1, 0,
  0, 0, 1
);
```

You can also mark a range with `valafmt-ignore-start` and `valafmt-ignore-end`

## Ignoring files

Anything matched by `.ignore`, `.gitignore`, `.git/info/exclude` or your global
git excludes are automatically ignored.

## Configuration

valafmt reads `indent_size` and `max_line_length` from `.editorconfig`
(`indent_size = tab` uses `tab_width`). The `--indent` and `--width` flags take
precedence over it. When formatting stdin, `.editorconfig` is resolved as if
the input were a `stdin.vala` file in the current directory.
