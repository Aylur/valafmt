# valafmt

An opinionated code formatter for [Vala](https://vala.dev).

> [!NOTE]
>
> _This project is vibe coded_. I feel like formatting is a trivial and boring
> problem that has been solved numerous times and it is completely fine to let
> an LLM generate another one. I was not happy with uncrustify or elementary's
> formatter, I just wanted the [Prettier](https://prettier.io/) for Vala, but
> did not have the will or the time to write it myself.

## Known limitation

`#if` / `#else` preprocessor branches are preserved verbatim, and files whose
branches each form valid syntax on their own format fine. A file where the
branches are _alternative fragments_ (e.g. two competing method signatures
around an `#else`) does not concatenate into parseable code; valafmt refuses
such files rather than guessing. In the corpus this affects ~0.3% of files.

## Building

```sh
cargo build --release        # binary in target/release/valafmt
```

## TODO:

```
opening braces after case in switch blocks should be on the same line

  in /home/demeter/Projects/astal/lib/notifd/src/cli.vala for example:

  case "int":
      {
          hints.set(name, new Variant.int32(int.parse(parts[2])));
          return null;
      }

  should be

  case "int": {
      hints.set(name, new Variant.int32(int.parse(parts[2])));
      return null;
  }
```

### Ignore

```
// valafmt-ignore


// valafmt-ignore:start

// valafmt-ignore:end
```
