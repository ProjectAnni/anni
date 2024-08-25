# Annim

## Installation

```bash
cargo install seaography-cli
```

## Code generation

```bash
sea-orm-cli generate entity --database-url 'sqlite:///tmp/annim.sqlite?mode=rwc'
# --seaography --model-extra-derives async_graphql::SimpleObject
seaography-cli -f=axum ./ src/entities 'sqlite:///tmp/annim.sqlite' annim
```
