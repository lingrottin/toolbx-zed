# toolbx-zed

[![Ferris.love badge](https://ferris.love/badge/lingrottin/toolbx-zed?variant=mini)](https://ferris.love/lingrottin/toolbx-zed)

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

### Zed Preview

Set the `TOOLBX_ZED_FLATPAK_PREVIEW` environment variable to make `toolbx-zed` use the preview version of Zed.(`dev.zed.Zed-Preview`) **(Flatpak only)**

```bash
TOOLBX_ZED_FLATPAK_PREVIEW=1 zed [path]
```

To persist this behavior, add the folowing to your `~/.bashrc`:

```bash
alias zed="env TOOLBX_ZED_FLATPAK_PREVIEW=1 zed"
```

## How it works

- Once you call `toolbx-zed` within a Toolbx container, it would look for `/run/.containerenv` (to get the container id) and `/run/.toolboxenv` (to determine if that was a Toolbx container).
- Then it will override Zed's PATH and create symlinks named `ssh` and `sftp` pointing to `toolbx-zed` itself.
- After that, it calls Zed with a fake SSH URL (like `ssh://1000@<container_id>.toolbx/var/home/user/some/path`), then our fake `ssh` and `sftp` would know that your user id is `1000` and the container id is `<container_id>`.
- Then the fake `ssh` would call `podman exec` routing its stdio to Zed.
- Finally Zed would think it is operating on a SSH machine, for everything is exactly identical to what Zed expects OpenSSH to do.


### Disallow escaping the sandbox

As you might see, `toolbx-zed` works by overriding Zed's PATH to force it use our fake OpenSSH binaries.

However, when Zed tries to escape the Flatpak sandbox, Zed would spawn itself outside the Flatpak sandbox, loading the environment variables from the User. (See [Zed's source](https://github.com/zed-industries/zed/blob/0678d61f085e48308b06fc58172fda6764393453/crates/cli/src/main.rs#L980) and [Flatpak Zed readme](https://github.com/flathub/dev.zed.Zed#environment-variables)) This makes it nearly impossible to override Zed's attempts to call `ssh` and `sftp`, without adding our fake binaries to **your** PATH, (instead of a temporary PATH only inserted during a Zed session.) So `toolbx-zed` will explicitly disallow Zed to escape the Flatpak sandbox by setting the `ZED_FLATPAK_NO_ESCAPE` variable. 

Don't worry! This would basically affect nothing, since, after all, Zed thinks it's developing "remotely", so the environment on the "client" does not matter much.

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
