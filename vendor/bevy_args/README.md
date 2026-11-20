# bevy_args 🧩
[![test](https://github.com/mosure/bevy_args/workflows/test/badge.svg)](https://github.com/Mosure/bevy_args/actions?query=workflow%3Atest)
[![GitHub License](https://img.shields.io/github/license/mosure/bevy_args)](https://raw.githubusercontent.com/mosure/bevy_args/main/LICENSE)
[![GitHub Last Commit](https://img.shields.io/github/last-commit/mosure/bevy_args)](https://github.com/mosure/bevy_args)
[![GitHub Releases](https://img.shields.io/github/v/release/mosure/bevy_args?include_prereleases&sort=semver)](https://github.com/mosure/bevy_args/releases)
[![GitHub Issues](https://img.shields.io/github/issues/mosure/bevy_args)](https://github.com/mosure/bevy_args/issues)
[![Average time to resolve an issue](https://isitmaintained.com/badge/resolution/mosure/bevy_args.svg)](http://isitmaintained.com/project/mosure/bevy_args)
[![crates.io](https://img.shields.io/crates/v/bevy_args.svg)](https://crates.io/crates/bevy_args)

bevy plugin to parse command line arguments and URL query parameters into resources


## command line arguments
`cargo run --example=minimal -- --my-string hello --my-int 42 --my-bool --my-enum another-value`

## URL query parameters
`http://localhost:8080/?my_string=hello&my_int=42&my_bool=true&my_enum=AnotherValue`


## minimal example

```rust
use bevy_args::BevyArgsPlugin;


#[derive(
    Default,
    Debug,
    Resource,
    Serialize,
    Deserialize,
    Parser,
)]
#[command(about = "a minimal example of bevy_args", version, long_about = None)]
pub struct MinimalArgs {
    #[arg(long, default_value = "hello")]
    pub my_string: String,

    #[arg(long, default_value = "42")]
    pub my_int: i32,

    #[arg(long)]
    pub my_bool: bool,
}


pub fn main() {
    let mut app = App::new();

    app.add_plugins(BevyArgsPlugin::<MinimalArgs>::default());
    app.add_systems(Startup, print_minimal_args);

    app.run();
}

fn print_minimal_args(args: Res<MinimalArgs>) {
    println!("{:?}", *args);
}
```


## compatible bevy versions

| `bevy_args` | `bevy` |
| :--         | :--    |
| `4.0`       | `0.19` |
| `3.0`       | `0.18` |
| `2.0`       | `0.17` |
| `1.8`       | `0.16` |
| `1.7`       | `0.15` |
| `1.5`       | `0.14` |
| `1.3`       | `0.13` |
| `1.0`       | `0.12` |
