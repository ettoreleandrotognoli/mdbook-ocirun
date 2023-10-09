[![Workflow Status](https://github.com/ettoreleandrotognoli/mdbook-ocirun/actions/workflows/main.yml/badge.svg)](https://github.com/ettoreleandrotognoli/mdbook-ocirun/actions?query=workflow%3A%22main%22)
[![Crates.io](https://img.shields.io/crates/l/mdbook-ocirun)](./LICENSE)
[![crates.io](https://img.shields.io/crates/v/mdbook-ocirun.svg)](https://crates.io/crates/mdbook-ocirun)

# mdbook-ocirun

This is a preprocessor for the [rust-lang mdbook](https://github.com/rust-lang/mdBook) project.
This allows to run arbitrary commands and code snippets inside containers and include the output of them within the markdown file.

## Getting started

```sh
cargo install mdbook-ocirun
```

You also have to activate the preprocessor, put this in your `book.toml` file:

```toml
[preprocessor.ocirun]
```

## Running arbitrary commands

Let's say we have these two files:

Markdown file: file.md

```markdown
# Title

<!-- ocirun alpine seq 1 10 -->

<!-- ocirun python python script.py -->

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

### Details

When the pattern `<!-- ocirun <image> $1 -->\n` or `<!-- ocirun <image> $1 -->` is encountered, the command `$1` will be run using the container like this: `docker run <image> $1`.
Also the working directory is the directory where the pattern was found (not root).
The command invoked must take no inputs (stdin is not used), but a list of command lines arguments and must produce output in stdout, stderr is ignored.

### Examples

The following is valid:

````markdown

<!-- ocirun python python generate_table.py -->

```rust
<!-- ocirun alpine cat program.rs -->
```

```diff
<!-- ocirun alpine diff a.rs b.rs -->
```

```console
<!-- ocirun alpine ls -l . -->
```

## Example of inline use inside a table

````markdown
Item | Price | # In stock
---|---|---
Juicy Apples | <!-- ocirun node node price.mjs apples --> | *<!-- ocirun node node quantity.mjs apples  -->*
Bananas | *<!-- ocirun node node price.mjs bananas -->* | <!-- ocirun node node quantity.mjs bananas -->
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


## Running Code Snippets


First you have to config how to run your snippets, here one example for python:

```toml
[[preprocessor.ocirun.langs]]
name = "python"
image = "python"
command = ["python", "source"]
```

So when we found a snippet like:

````markdown
```python,ocirun
print('Hello World')
```
````

We will append the output:

````markdown
```console,success
Hello World
```
````

The final result should be something like this:

```python,ocirun
print('Hello World')
```
```console,success
Hello World
```

## Contributors

I would like to thank [@FauconFan](https://github.com/FauconFan) for his work on [mdbook-cmdrun](https://github.com/FauconFan/mdbook-cmdrun) that was my start for this project.

Current version: 0.2.0  
License: MIT
