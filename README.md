# toolbx-zed

[![Ferris.love badge](https://ferris.love/badge/lingrottin/toolbx-zed?show=call_fn%2Cdef_fn%2Cdef_struct%2Cdef_method%2Cdef_trait&variant=mini)](https://ferris.love/lingrottin/toolbx-zed)

A seamless integration tool to use the [Zed](https://zed.dev/) editor within [Toolbx](https://containertoolbx.org/) containers. 

## Installation

The easiest way to install `toolbx-zed` is via the automated installation script. 

```bash
curl -sL https://raw.githubusercontent.com/lingrottin/toolbx-zed/main/install.sh | bash
```

### Force Build from Source

If you want to force the installer to build from source instead of downloading a pre-compiled binary, set the `TOOLBX_ZED_BUILD_FROM_SOURCE` environment variable:

```bash
curl -sL https://raw.githubusercontent.com/lingrottin/toolbx-zed/main/install.sh | env TOOLBX_ZED_BUILD_FROM_SOURCE=1 bash
```
*(Note: Building from source requires [the Rust toolchain](https://rustup.rs) to be installed on your system).*

## Usage

Once installed, a `zed` command will be available in your configured PATH.

1. Enter your toolbox container:
   ```bash
   toolbox enter <container>
   ```

2. Open a project or file using Zed:
   ```bash
   zed <some_path>
   ```

## Notes for Flatpak Users

If you are using the Flatpak version of Zed (`dev.zed.Zed`), it requires access to your home directory to interact properly with `toolbx-zed`. 

This is usually enabled by default, but if you encounter permission issues, ensure that Zed has the `--filesystem=home` permission granted. You can manage this using [Flatseal](https://flathub.org/apps/com.github.tchx84.Flatseal) or via the command line:

```bash
flatpak override --user --filesystem=home dev.zed.Zed
```

## Debugging

Logs will be available at `/run/user/$(id -u)/toolbx-zed.log`, once you set `TOOLBX_ZED_DEBUG` environment variable.

Note that it might be hard debugging the release binaries, especially when it is called by Zed as `ssh` or `sftp`, since it's hard to set environment variables in those cases. Please consider building from source in debug mode instead.

If it is called by Zed in the flatpak sandbox, logs would appear in the sandbox. Use the following command to access the logs:

```bash
flatpak run --command=sh dev.zed.Zed -c "cat /run/user/$(id -u)/toolbx-zed.log"

# or if you want to follow the logs in real-time
flatpak run --command=sh dev.zed.Zed -c "tail -f /run/user/$(id -u)/toolbx-zed.log"
```

## License

This project is open-source and available under the [MIT license](./LICENSE).

## Acknowledgement

This project was inspired by [toolbox-vscode](https://github.com/owtaylor/toolbox-vscode).
