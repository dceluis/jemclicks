# jemclicks

# Installation

```
cargo build src/main.rs
```

and then copy the binary to your bin path

```
cp target/debug/jemclicks ~/bin/
```

# Usage

Running jemclicks involves two parts.

Starting the jemclicks server:

```
$ sudo jemclicks -d <device_number>
```

Figure out your keyboard's device number by running jemclicks without any arguments:

```
$ sudo jemclicks
```

Note: root privileges are required since grabbing input devices is a privileged action.

Enabling/disabling jemclicks mouse input:

```
$ jemclicks enable
```

```
$ jemclicks disable
```

Typing and executing the enable command from a terminal everytime you need to toggle the mouse is not practical, though.
Ideally, you would have a way of triggering the command from an appropriate shortcut depending on your environment.

Examples:

i3mw
```
bindsym $mod4+x exec jemclicks enable
bindsym $mod4+z exec jemclicks disable
```

Note: You can also disable the jemclicks mouse with the `quit` button, see below for the default key bindings.

My original use case was to use a foot pedal as the toggle, a-la vim-clutch. I'm using kmonad to remap the pedal, with the following configuration:

```
(defcfg
  input  (device-file "/dev/input/by-id/usb-1a86_e026-event-kbd")
  output (uinput-sink "Remapped Mouse Pedal - Kmonad")
  cmp-seq cmp

  fallthrough true

  allow-cmd true
)

(defsrc
  a
)

(defalias
  btog (cmd-button "jemclicks enable" "jemclicks disable")
)

(deflayer main
  @btog
)
```

# Default keybindings

Up: `I`
Left: `J`
Down: `K`
Left: `L`

Left-click: `S`
Middle-click: `D`
Right-click: `F`

Quit: `Q`

# Notes

* The configuration file is not currently functional. If you want to change the keybindings, you can edit the src file and build the executable again.
