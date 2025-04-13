# Storage API

This is an implementation of the `Storage` API, a better version of the `Allocator` API, and data structures made for them including

- `Box`
- `Vec`
- `String`

## How is it better than `Allocator`?

`Storage`s have an associated `Handle` type so allocations dont need to be represented by a pointer, which allows `Storage`s to allocate from a buffer they store inline

Instead of having `Vec` and `ArrayVec` as 2 seperate data structures they can be merged together, only using different `Storage`s
