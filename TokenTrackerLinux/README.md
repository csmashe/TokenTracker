# TokenTracker Linux Client

This is a local x86_64 Linux desktop client for TokenTracker. The packaged path targets Arch Linux + KDE Plasma; Fedora can build and run the client directly from a repository checkout. It is intended for personal local use, not public release distribution.

> **Packaging scope:** This PKGBUILD builds from a local repository checkout. It is not ready for AUR publication or clean-chroot builds, and the build does not promise byte-identical artifacts.

## Build and install

### Arch Linux package

```bash
cd TokenTrackerLinux/packaging/arch/tokentracker-linux
makepkg -si
```

### Fedora local build

Install the Tauri/WebKitGTK build dependencies and Rust toolchain:

```bash
sudo dnf install \
  webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel \
  gcc gcc-c++ make pkgconf-pkg-config rust cargo
```

Build the self-contained runtime and Tauri executable from the repository root:

```bash
npm ci
npm --prefix dashboard ci
npm run dashboard:build
npm --prefix TokenTrackerLinux ci
npm --prefix TokenTrackerLinux run bundle:node
npm --prefix TokenTrackerLinux run build
```

Run the checkout build without installing system files:

```bash
./TokenTrackerLinux/src-tauri/target/release/tokentracker-linux
```

Install it for the current user, including an application-menu entry and icon:

```bash
npm run linux:install:local
```

The default installation prefix is `~/.local`; override it with `TOKENTRACKER_LINUX_PREFIX=/absolute/path`. Remove the local installation with:

```bash
npm run linux:uninstall:local
```

On GNOME, enable an AppIndicator extension so the close-to-tray controls remain accessible.

The client disables WebKitGTK's DMA-BUF renderer by default to avoid blank or aborted webviews on affected Wayland/NVIDIA systems. Set `WEBKIT_DISABLE_DMABUF_RENDERER=0` before launching if you explicitly want to retry the accelerated renderer.

## Run

Start **TokenTracker** from the KDE application launcher, or run:

```bash
tokentracker-linux
```

The app starts a bundled local TokenTracker server, opens the existing dashboard in a Tauri window, and keeps a system tray icon alive.

The Linux shell starts the server in embedded safe mode. It reads the existing TokenTracker data directory but does not automatically initialize TokenTracker, replace `~/.tokentracker/tracker/app`, repair AI-tool integrations, or stop an unrelated process that owns its requested port. New users should run `npx tokentracker-cli init` separately before launching the desktop client; until they do, the dashboard stays empty and the server log records the reason.

Server output goes to `${XDG_STATE_HOME:-~/.local/state}/tokentracker/server.log`.

OAuth completes through the dashboard's loopback HTTP callback. The Linux package does not register the unimplemented `tokentracker://` desktop protocol.

## Window behavior

- Closing the window hides it to the tray.
- Tray **Open Dashboard** restores the window.
- Tray **Quit** stops the bundled Node server and exits the app.

## Uninstall

```bash
sudo pacman -R tokentracker-linux
```
