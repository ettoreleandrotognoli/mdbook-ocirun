[![Workflow Status](https://github.com/FauconFan/mdbook-ocirun/actions/workflows/main.yml/badge.svg)](https://github.com/FauconFan/mdbook-ocirun/actions?query=workflow%3A%22main%22)
![Crates.io](https://img.shields.io/crates/l/mdbook-ocirun)

# mdbook-ocirun

This is a preprocessor for the [rust-lang mdbook](https://github.com/rust-lang/mdBook) project. This allows to run arbitrary (shell) commands and include the output of these commands within the markdown file.

## Getting started

```sh
cargo install mdbook-ocirun
```

You also have to activate the preprocessor, put this in your `book.toml` file:
```toml
[preprocessor.ocirun]
```

## How to

Let's say we have these two files:

Markdown file: file.md
```markdown
# Title

<!-- ocirun seq 1 10 -->

<!-- ocirun python3 script.py -->

```

Python file: script.py
```python
def main():
    print("## Generated subtitle")
    print("  This comes from the script.py file")
    print("  Since I'm in a scripting language,")
    print("  I can compute whatever I want")

if __name__ == "__main__":
    main()

```

The preprocessor will call seq then python3, and will produce the resulting file:

```markdown
# Title

1
2
3
4
5
6
7
8
9
10

## Generated subtitle
  This comes from the script.py file
  Since I'm in a scripting language,
  I can compute whatever I want


```

## Details

When the pattern `<!-- ocirun $1 -->\n` or `<!-- ocirun $1 -->` is encountered, the command `$1` will be run using the shell `sh` like this: `sh -c $1`.
Also the working directory is the directory where the pattern was found (not root).
The command invoked must take no inputs (stdin is not used), but a list of command lines arguments and must produce output in stdout, stderr is ignored.

As of July 2023, mdbook-ocirun runs on Windows platforms using the `cmd` shell!

## Examples

The following is valid:

````markdown

<!-- ocirun python3 generate_table.py -->

```rust
<!-- ocirun cat program.rs -->
```

```diff
<!-- ocirun diff a.rs b.rs -->
```

```console
<!-- ocirun ls -l . -->
```

## Example of inline use inside a table
````markdown
Item | Price | # In stock
---|---|---
Juicy Apples | <!-- ocirun node price.mjs apples --> | *<!-- ocirun node quantity.mjs apples  -->*
Bananas | *<!-- ocirun node price.mjs bananas -->* | <!-- ocirun node quantity.mjs bananas -->
````

Which gets rendered as:
````markdown
Item | Price | # In stock
---|---|---
Juicy Apples | 1.99 | *7*
Bananas | *1.89* | 5234
````

Some more examples are implemented, and are used as regression tests. You can find them [here](https://github.com/FauconFan/mdbook-ocirun/tree/master/tests/regression/).
At the moment of writing, there are examples using:
- Shell
- Bash script
- Batch script
- Python3
- Node
- Rust


## Contributors

I would like to thank [@exsjabe](https://github.com/exsjabe) for his valuable help with integrating Windows support and inline ocirun calls.

Current version: 0.6.0  
License: MIT
