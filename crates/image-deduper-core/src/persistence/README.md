# Persistence Module

This directory contains the SQLite-based persistence implementation for the image deduplication system.

## Overview

The persistence module stores image metadata, file paths, and hash information to enable efficient lookups and comparisons. It provides a simple API for storing and retrieving images from an SQLite database.

## Documentation

Complete documentation is available as doc comments in the source code:

- [`mod.rs`](./mod.rs) - Module overview, examples, and API documentation
- [`db.rs`](./db.rs) - Database implementation
- [`models.rs`](./models.rs) - Data models
- [`error.rs`](./error.rs) - Error types

## Key Features

- SQLite database with optimized configuration
- Efficient indexing for fast lookups by path and hash values
- Schema versioning and migration support
- Comprehensive error handling

## Usage

See the doc comments in [`mod.rs`](./mod.rs) for usage examples.

You can also generate the complete documentation using:

```bash
cargo doc --open
```
