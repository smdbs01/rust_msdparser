# Rust MSDParser

This is a port of the [MSD Parser](https://github.com/garcia/msdparser/) library by [Garcia](https://github.com/garcia) to Rust. This library exposes APIs similar to its Python origin, namely the `parse_msd` function.

```rust
fn parse_msd<R: Read>(input: R, escapes: bool, ignore_stray_text: bool) -> MSDParser<R>;
```

The returned struct is an iterator that yields `Result<MSDParameter, MSDParserError>`, where `MSDParameter` is a key-value pair. The keys and values can be accessed by using `.key()` and `.value()` respectively.

See example below to get an idea of how it works.

# Usage

```rust
use msdparser::{MSDParameter, parse_msd};
use std::error::Error;
use std::vec::Vec;

let example_input = b"\
#VERSION:0.83;
#TITLE:Springtime;
#SUBTITLE:;
#ARTIST:Kommisar;";
let mut result: Vec<MSDParameter> = Vec::new();

// here we set `escapes` to true and `ignore_stray_text` to false
// which is the default value in the original python library
for parameter in parse_msd(example_input.as_ref(), true, false) {
    match parameter {
       Ok(parameter) => result.push(parameter),
       Err(e) => panic!("{}", e), // = MSDParserError
    }   
}

assert_eq!(result.len(), 4);
assert_eq!(result[0].key().unwrap(), "VERSION".to_string());
assert_eq!(result[1].value().unwrap(), "Springtime".to_string());
assert_eq!(result[2].value().unwrap(), "".to_string());
assert_eq!(result[3].key().unwrap(), "ARTIST".to_string());
```

# Installation

1. Add the following to your `Cargo.toml`:

```toml
[dependencies]
msdparser = { version = "0.1.0", git = "https://github.com/smdbs01/rust_msdparser.git" }
```

**Or**

2. Use `Cargo add msdparser`.

# Contribute

This is my first project using Rust, so it is very likely that the codebase is not "rusty" enough. So, if you find any bugs or suggestions, please feel free to open an issue or PR.