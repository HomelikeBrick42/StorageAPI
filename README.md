# Storage API

[![Latest Version](https://img.shields.io/crates/v/storage_api.svg)](https://crates.io/crates/storage_api)
[![Rust Documentation](https://docs.rs/storage_api/badge.svg)](https://docs.rs/storage_api)
![GitHub license](https://img.shields.io/badge/license-MIT-blue.svg)

Note: This crate currently requires using nightly by default, unless you make `default-features = false`, this is so `Box` can support `T: ?Sized`

This is an implementation of the `Storage` API, a better version of the `Allocator` API, and data structures made for them including

- `Box`
- `Vec`
- `VecDeque`
- `String`

## How is it better than `Allocator`?

`Storage`s have an associated `Handle` type so allocations dont need to be represented by a pointer, which allows `Storage`s to allocate from a buffer they store inline

Instead of having `Vec` and `ArrayVec` as 2 seperate data structures they can be merged together, only using different `Storage`s
