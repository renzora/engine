# `glam_matrix_extras`

[![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/Jondolf/glam_matrix_extras#license)
[![ci](https://github.com/Jondolf/glam_matrix_extras/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Jondolf/glam_matrix_extras/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/glam_matrix_extras?label=crates.io)](https://crates.io/crates/glam_matrix_extras)
[![docs.rs](https://img.shields.io/docsrs/glam_matrix_extras?label=docs.rs)](https://docs.rs/glam_matrix_extras)

Matrix types and utilities for [`glam`].

[`glam`]: https://docs.rs/glam/latest/glam/

## Features

- `SquareMatExt` extension trait with useful helpers like `is_symmetric`, `inverse_or_zero`, and `diagonal`
- Rectangular matrices
    - [x] 2x3 matrices: `Mat23`, `DMat23`
    - [x] 3x2 matrices: `Mat32`, `DMat32`
- Symmetric matrices
    - [x] Symmetric 2x2 matrices: `SymmetricMat2`, `SymmetricDMat2`
    - [x] Symmetric 3x3 matrices: `SymmetricMat3`, `SymmetricDMat3`
    - [x] Symmetric 4x4 matrices: `SymmetricMat4`, `SymmetricDMat4`
    - [x] Symmetric 5x5 matrices: `SymmetricMat5`, `SymmetricDMat5`
    - [x] Symmetric 6x6 matrices: `SymmetricMat6`, `SymmetricDMat6`
- Eigen decompositions of symmetric matrices
    - [x] 2x2: `SymmetricEigen2`
    - [x] 3x3: `SymmetricEigen3`

## Supported Glam Versions

| `glam` | `bevy_reflect` | `glam_matrix_extras` |
| ------ | -------------- | -------------------- |
| 0.32   | 0.19-rc        | main                 |
| 0.30   | 0.18           | 0.2                  |
| 0.30   | 0.17           | 0.1                  |

## License

`glam_matrix_extras` is free and open source. All code in this repository is dual-licensed under either:

- MIT License ([LICENSE-MIT](/LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](/LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.
