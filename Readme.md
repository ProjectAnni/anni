# Anniversary

> 護りたい  
> あなたとの毎日と  
> これからの未来へ続く道を  
> たくさんの愛に満ちた温もり  
> そう　それは　陽だまりに咲くリバティ  
> 束ねては　贈る　ひとひら  
> Anniversary

## Child Projects

- anni: Cli-tool with all features.
- anni-flac: FLAC parsing library.
- anni-repo: Music-repository related works.
- anni-utils: Utilities used by other projects.
- anni-versary: Music backend implementation.

## Use Cases

- Print FLAC tags to stdout directly.
- Perform FLAC tag content check.
- Helper to output `shnsplit` command.
- Read `cue` file and output commands to update FLAC file tags.

## Features/TODOs

- [ ] Music backend focuses on `flac` format.
- [ ] Built-in metaflac alternative.
    - [x] `--list`
        - [ ] `--block-number`
        - [ ] `--block-type`
    - [x] `--export-tags-to=-`
    - [ ] `--import-picture-from`
    - [ ] `--export-picture-to`
- [ ] Built-in Google Drive support.
- [ ] Airsonic API support.