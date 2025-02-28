# Geode Command Line
Command-line utilities for working with [Geode SDK](https://geode-sdk.org).

For more information see its [page on the docs](https://docs.geode-sdk.org/getting-started/geode-cli).

## Usage
The CLI is typically invoked for you by Geode's build system, but it does have some standalone features:

``` bash
# Walks you through creating a Geode mod via a template
geode new

# Runs Geometry Dash from the default profile
geode run

# Installs the sdk. For more info see the docs
geode sdk install

# Uploads a mod to the index. For more info see the docs
geode index mods create
```

## Installation
*(For more in depth information see the docs.)*

### Windows (scoop)
```
scoop bucket add extras
scoop install geode-sdk-cli
```

### Windows (Winget)
> This may be out of date. Sorry!
```
winget install GeodeSDK.GeodeCLI
```

### MacOS (brew)
```
brew install geode-sdk/geode/geode-cli
```

### Linux
We provide linux binaries [in every release](https://github.com/geode-sdk/cli/releases/latest).

### Arch Linux (Unoffical AUR package)
> **Note**: This package is unofficial and not maintained by us. Use at your own risk.

[geode-cli-bin](https://aur.archlinux.org/packages/geode-cli-bin)

### From source
> **Note**: This require a [local rust installation](https://www.rust-lang.org/tools/install), and can take a very long time
```
cargo install --git https://github.com/geode-sdk/cli
```
