# homescreen

Launcher process: a slim Smithay nest (`compositor` without the `session`
feature) presented to the host as one `zwlr_layer_shell_v1` Background surface.
The host client is a local mecha-wayland `main` (`vendor/mecha-wayland`):
`App` + `io-ring` + generated `wayland` + `renderer`. One Background
surface and one EGL context. Smithay draws nest tiles into the swapchain,
then mecha-wayland draws chrome into the same buffer. Clone:

```sh
git clone --branch main https://github.com/mecha-org/mecha-wayland.git vendor/mecha-wayland
```

Needs nightly Cargo.

```sh
# from a session that already has WAYLAND_DISPLAY (mechanix-comp, niri, …)
cargo run -p homescreen
```

The nest binds `wayland-widget-0` (then `-1`…). Widgets are ordinary xdg
clients on that socket. Icons launch on the host `WAYLAND_DISPLAY`.

Config: `$XDG_CONFIG_HOME/mechanix/homescreen.toml` (or `MECHANIX_HOME_CONFIG`).

Gestures: swipe changes page (widgets do not see it). Tap a widget to click
it; tap an icon to launch. Long-press enters edit mode. Hold-and-drag lifts
the item (neighbors reflow around a hole). Long-press then drag a handle
resizes; the frame follows the pointer, the widget snaps at mid-cell. Add
slots only by editing the file.
