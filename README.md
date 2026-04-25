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

If you are using the Flatpak version of Zed (`dev.zed.Zed` or `dev.zed.Zed-Preview`), it requires access to your home directory to interact properly with `toolbx-zed`. 

This is usually enabled by default, but if you encounter issues like `ssh: Could not resolve hostname <a very long alphanumeric string>.toolbx: Name or service not known`,
ensure that Zed has the `--filesystem=home` permission granted. You can manage this using [Flatseal](https://flathub.org/apps/com.github.tchx84.Flatseal) or via the command line:

```bash
flatpak override --user --filesystem=home dev.zed.Zed
```

When using Flatpak Zed, `toolbx-zed` will explicitly disallow Zed to escape the Flatpak sandbox. This is because when Zed tries to do that, it's nearly impossible to make Zed use
our fake `ssh` and `sftp` without adding them to **your** PATH, (instead of a temporary PATH only inserted during a Zed session.) Don't worry! This would basically affect nothing
since after all Zed thinks it's developing "remotely", so the environment on the "client" does not matter at all.

### Zed Preview

Set the `TOOLBX_ZED_FLATPAK_PREVIEW` environment variable to make `toolbx-zed` use the preview version of Zed. **(Flatpak only)**

```bash
TOOLBX_ZED_FLATPAK_PREVIEW=1 zed [path]
```

To persist this behavior, add the folowing to your `~/.bashrc`:

```bash
alias zed="env TOOLBX_ZED_FLATPAK_PREVIEW=1 zed"
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
