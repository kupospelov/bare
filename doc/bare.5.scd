bare(5)

# NAME

bare - configuration file

# DESCRIPTION

The configuration file is located at *$XDG\_CONFIG\_HOME/bare/config.toml* or at
*~/.config/bare/config.toml*.

# BAR

The *\[bar\]* section configures the bar.

*font*=_font_
	Set font to _font_, as a Pango font description.

	Default: "Sans Bold 9"

*width*=_px_
	Set the width of the bar in pixels.

	Default: 28

*interval*=_interval_
	Polling interval in seconds.

	Default: 10

*separator*=_px_
	Set the vertical space between adjacent blocks in pixels.

	Default: 14

*blocks*=_\[name, ...\]_
	Define the blocks to render, from top to bottom. Each entry has the form
	_type.name_. See *BLOCKS* for more information.

	Default: \["volume.0", "battery.0", "time.0"\]

*color*=_colors_
	Default colors used by blocks that do not override them. See *COLORS* for
	more information.

# WORKSPACE

The *\[workspace\]* section accepts options described in *BOX MODEL* with the
following defaults: *height*=26, *borders*=\[1, 1, 1, 1\], *margins*=\[0, 2, 2,
0\].

In addition to that, workspaces have the *active*, *inactive*, and *urgent*
states. Each state defines a color section, e.g. *\[workspace.urgent.color\]*.
See *COLORS* for more information.

# BLOCKS

Blocks are configured as named instances of a block type. The available types
are *cpu*, *wireless*, *volume*, *battery*, and *time*. Instances are defined in
*\[type.<name>\]* sections and referenced in the *\[bar.blocks\]*.

Each block section accepts options described in *BOX MODEL*, a *color* section
described in *COLORS*, and a *format* option specific to the block type.

If the *color* section is omitted, the block inherits the colors configured in
*\[bar.color\]*. If a block supports states, the colors can be overridden
per-state.

*format* is an array of strings. Each string is either a label or a placeholder.
Labels longer than 2 characters are rendered with a smaller font size to make
them aligned with regular 2-character blocks. Placeholders are always rendered
with the bar font size to avoid vertical shifts.

## CPU

*\[cpu.<name>\]*
	Display the CPU status.

	*format*=_\[token, ...\]_

		Default: \["CPU", "\[usage\]"\]

		The following placeholders are recognized:

		*\[usage\]*
			Usage percentage.

## Wireless

*\[wireless.<name>\]*
	Display the status of a wireless interface.

	*interface*=_name_
		Name of the interface.

		Default: "wlan0"

	*format*=_\[token, ...\]_

		Default: \["NET", "\[quality\]"\]

		The following placeholders are recognized:

		*\[quality\]*
			Signal quality.

## Volume

*\[volume.<name>\]*
	Display the volume of the default PipeWire audio sink.

	*format*=_\[token, ...\]_

		Default: \["VOL", "\[volume\]"\]

		The following placeholders are recognized:

		*\[volume\]*
			Current volume percentage of the default audio sink.

	The volume has the *muted* state. The state defines a color section, e.g.
	*\[muted.color\]*. See *COLORS* for more information.

## Battery

*\[battery.<name>\]*
	Display the battery capacity.

	*path*=_path_
		Path to the _uevent_ file in sysfs.

		Default: "/sys/class/power_supply/BAT0/uevent"

	*format*=_\[token, ...\]_

		Default: \["BAT", "\[capacity\]"\]

		The following placeholders are recognized:

		*\[capacity\]*
			Current battery capacity percentage.

	The battery has the *charging*, *full*, *idle*, and *unknown* states. Each
	state defines a color section, e.g. *\[charging.color\]*. See *COLORS* for
	more information.

## Time

*\[time.<name>\]*
	Display the current local time.

	*format*=_\[token, ...\]_

		Default: \["\[hour\]", "\[minute\]"\]

		The following placeholders are recognized:

		*\[hour\]*
			Current hour, zero-padded to two digits.

		*\[minute\]*
			Current minute, zero-padded to two digits.

		*\[day\]*
			Current day of the month, zero-padded to two digits.

		*\[month\]*
			Current month, zero-padded to two digits.

# BOX MODEL

The following options control the size and spacing of a block. They are shared
by *\[workspace\]* and all block types.

*height*=_px_
	The height of the block in pixels. The value includes borders. If omitted,
	the block is sized to fit its contents.

	Default: 0

*borders*=_\[top, right, bottom, left\]_
	Set the border thickness on each edge in pixels.

	Default: \[0, 0, 0, 0\]

*margins*=_\[top, right, bottom, left\]_
	Set the margin of the block in pixels.

	Default: \[0, 0, 0, 0\]

# COLORS

Colors are specified as _#rrggbb_ hex strings. Each color section has the
following keys: *text*, *background*, and *border*.

Unless explicitly set, blocks inherit colors from *\[bar.color\]*.

The default colors are:

[[ *Section*
:[ *text*
:[ *background*
:[ *border*
|[ *\[bar.color\]*
:[ #646464
:[ #000000
:[ #000000
|[ *\[workspace.active.color\]*
:[ #ffffff
:[ #285577
:[ #4c7899
|[ *\[workspace.inactive.color\]*
:[ #888888
:[ #222222
:[ #333333
|[ *\[workspace.urgent.color\]*
:[ #ffffff
:[ #77282d
:[ #994c4c
|[ *\[volume.<name>.muted.color\]*
:[ #dfaf8f
:[ _inherited_
:[ _inherited_
|[ *\[battery.<name>.charging.color\]*
:[ #60b48a
:[ _inherited_
:[ _inherited_

# EXAMPLES

```
[bar]
font = "Sans Bold 9"
width = 28
separator = 14
blocks = [
	"volume.0",
	"battery.0",
	"time.date",
	"time.clock",
]

[workspace]
height = 26
borders = [1, 1, 1, 1]
margins = [0, 2, 2, 0]

[workspace.urgent.color]
background = "#77282d"
border = "#994c4c"

[volume.0]
format = ["VOL", "[volume]"]

[volume.0.muted.color]
text = "#dfaf8f"

[battery.0]
format = ["BAT", "[capacity]"]

[time.date]
format = ["[day]", "[month]"]

[time.clock]
format = ["[hour]", "[minute]"]
```

# SEE ALSO

*bare*(1)
