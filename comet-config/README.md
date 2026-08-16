# Comet on-device test session

mechanix-comp on the Mecha Comet. Phone layout plus a small client set.
**Do not reboot** (VT / greetd deadlocks). **Do not suspend** (deep sleep
does not resume on this board).

## Running

| | Role |
|---|---|
| `/usr/local/bin/compositor` | udev/DRM on `/dev/dri/card2` |
| waybar | top bar: launcher, taskbar, close, clock |
| nwg-drawer | full-screen app grid (☰ toggles) |
| stevia | OSK (`sm.puri.OSK0`) |
| gtk4-demo | usual test app |
| swayidle | 60s blank (`wlopm --off DSI-1`), 65s `gtklock` |
| gtklock | lock; stevia is the lock-screen keyboard |
| nwg-bar | long-press power menu |

Socket: `$XDG_RUNTIME_DIR/wayland-1` (name can change). From SSH: `mecha-run …`.

## Boot

greetd `initial_session` → `/usr/local/bin/mechanix-test-session`. That script
unsets `WAYLAND_DISPLAY`, sets `MECHA_DRM_DEVICE=/dev/dri/card2`, loops the
compositor, then starts the clients. `pkill -x compositor` hot-reloads.

`initial_session` runs **once per boot**. `systemctl restart greetd` after that
only starts agreety (no compositor). To pick up a new session script without
rebooting: kill the leftover `mechanix-test-session` and start
`/usr/local/bin/mechanix-test-session` again.

## Device

- Panel: DSI card2, 1080×1240, scale 2 → 540×620
- Power key: `platform-30370000.snvs:snvs-powerkey-event`
- logind: `HandlePowerKey=ignore`
- GTK4 needs GStreamer 1.28 from `/opt/gst128` (`LD_LIBRARY_PATH` in the session script). Login `profile.d` points at 1.26 (`/opt/gstreamer`), which is missing `gst_gl_display_x11_new_with_display`.
- SSH: `mecha@172.16.42.1` (USB gadget) or Wi-Fi (`192.168.1.61` on this LAN). Password `mecha`. Local ssh_config.d is broken; `ssh -F /dev/null`
- USB gadget: `usb-signaller` creates `usb0` / `172.16.42.1`. It starts at `basic.target` **before** dwc3 publishes the UDC, so it can lose the race and leave no USB SSH. If that happens: `sudo systemctl start usb-signaller` once `/sys/class/udc` is populated. Do not reboot.

## Gestures (compositor)

| | |
|---|---|
| Short power | lock then blank; press again wakes |
| Long power (~800 ms) | nwg-bar |

## Session shims

These are not compositor protocol. They hold the test image together.

- **`LD_LIBRARY_PATH=/opt/gst128/usr/lib64`** — GTK4 needs GStreamer 1.28; see Device above. Session-wide, so every client sees 1.28 ahead of the 1.26 login path.
- **gtklock `lock.ui`** — stock layout is ~653×620; we configure 540×620 and smithay rejects the commit. A crash leaves the session locked with no locker
- **waybar taskbar** — cannot nest dialogs; compositor `# TO REMOVE` skips parented toplevels
- **waybar ×** — no right-click on a finger; close is `wlrctl toplevel close state:active`
- **Font Awesome ☰** — `☰` is not in any font on the device
- **`mecha-run` / `mecha-session.env`** — SSH cannot see the socket otherwise
- **`wlopm`** — swayidle’s `set_mode` client; this build needs `--off DSI-1` (not `off`)

Compositor-side session stand-ins are marked `# SHELL-HELL` in the source.
