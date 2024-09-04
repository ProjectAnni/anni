# Annim

## Debug

```bash
export ANNIM_DATABASE_URL=postgres://postgres:password@localhost:5432/annim
export ANNIM_SEARCH_DIRECTORY=/tmp/tantivy
cargo run -p annim --release
```

## Installation

```bash
cargo install seaography-cli
```

## Code generation

```bash
sea-orm-cli generate entity --database-url 'sqlite:///tmp/annim.sqlite?mode=rwc'
```
