# bare

A lightweight vertical bar for Wayland compositors.

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
