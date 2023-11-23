# Contributing

## Releasing a new version for libraries

Current dependency graph looks like this:

1. Leaf: anni-common, anni-flac, anni-playback, anni-artist
2. anni-provider: anni-repo, anni-common, anni-flac
3. anni-repo: anni-common, anni-artist
4. anni-split: anni-common
5. anni-workspace: anni-repo, anni-common, anni-flac
6. annil: anni-flac, anni-repo, anni-provider

So if you want to upgrade a library depended by others, you need to upgrade the libraries directly depending on it too. Here is a list of libraries that need to be upgraded together:

1. anni-common: anni-provider, anni-repo, anni-split, anni-workspace
2. anni-flac: anni-provider, anni-workspace, annil
3. anni-playback: annix(in another repository)
4. anni-artist: anni-repo
5. anni-repo: anni-provider, anni-workspace, annil
6. anni-provider: annil

If the list was outdated, contact the maintainer to update it.
