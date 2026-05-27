# bare

A lightweight vertical bar for Wayland compositors.

<img width="1280" height="720" alt="bare" src="https://github.com/user-attachments/assets/9559e037-8a5b-489c-a00e-ffba882d56f0" />

## Inspiration

`bare` is a vertical bar focused on maximizing useful screen space and minimizing system resource usage.

It can be compared to the likes of `swaybar` or `i3bar` with a few key differences:
1. Vertical layout.
2. Built-in set of blocks (no dependency on `i3status`).

## Requirements

The compositor must support the following protocols:

- [ext-workspace-v1](https://wayland.app/protocols/ext-workspace-v1)
- [wlr-layer-shell-unstable-v1](https://wayland.app/protocols/wlr-layer-shell-unstable-v1)

You can follow the links to check yours.
