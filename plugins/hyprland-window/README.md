# Kidex

A plugin to quickly switch between windows in Hyprland WM.

## Usage

Simply search for the window by its name.

It's good to use this plugin before the `applications` plugin,
because then you can easily search for the window you want to switch to
or open a new window if it's not open yet.

## Configuration

```ron
// <Anyrun config directory>/hyprland-window.ron
Config(
  max_entries: 3,
  
  // It's good to tinker score threshold that is used in the fuzzy search, so if
  // you use this plugin together with `applications`, you don't accidentally
  // switch to a window that has a similar name to the application you want to
  // open.
  score_threshold: 50,
)
```