# Geode Command Line
Command-line utilities for working w/ Geode.

### ===> Installation <===
#### Method 1 (From a release zip):
1) Go to the [latest release](https://github.com/geode-sdk/cli/releases/latest) and download the zip for your platform
2) Unzip it
3) Add it to your path
  - Windows users: Go to `System Environment Variables` and\
    add the path to the folder the unzipped binary is in to the\
    `Path` variable, then reboot
  - MacOS users: Open up your Terminal and run `sudo vi /etc/paths`.\
    Once you're in, press `a` to start editing, go to the bottom of\
    the file and make a new line with the path to the folder where\
    the unzipped binary is. Once you're done, simply press `Esc`,\
    then type `:wq` (YOU MUST TYPE THE `:` OR THIS WILL NOT WORK).
  - Linux users: Simply do one of these:
      1. Add the path to the folder containing the downloaded binary\
         to your `$PATH`.
      2. Copy/move the binary to `/usr/bin`

#### Method 2 (From source):
1. Install `git` and `rust`
2. Open up a shell (e.g. Powershell, CMD, ZSH, Fish)
3. Run `git clone https://github.com/geode-sdk/cli`
4. Run `cd cli`
5. Run `cargo install --path . --release`

#### Method 3 - NIXOS USERS ONLY (From a Nix Flake):
- If you have flakes enabled, run `nix profile install github:geode-sdk/cli#default`
- If not, run the following:
```bash
git clone https://github.com/geode-sdk/cli
cd cli
nix-build
nix-env -i -f .
```
