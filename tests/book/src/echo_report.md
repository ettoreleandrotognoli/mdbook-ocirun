# Echo Report

<!-- ocirun python python echo.py oui non -->
<!-- ocirun python python echo.py another echo for fun -->
<!-- ocirun alpine yes 42 | head -n4 | sed -z "s/\n/  \n/g" -->

With ocirun:

```rust,ocirun
pub fn main() {
    println!("Hello World");
}
```

```python,ocirun
print("Hello World")
```

```ts,ocirun

console.log(`Hello World`);

```

Without ocirun:

```python
def main():
    pass
```

```c++

int main() {

}

```

```ts

console.log(`Hello World`);

```
