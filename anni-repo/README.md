# anni-repo

## Publish

```bash
# 1. Increase version
# 2. Build
wasm-pack build ./anni-repo --release --out-dir ../npm/repo --scope project-anni -- --features db-read
# 3. Rename package, from @project-anni/anni-repo to @project-anni/repo
# 4. Publish
cd npm/repo && npm publish --access public
```