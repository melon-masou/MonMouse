# MonMouse: seamless switching between multiple mouses on multiple monitors

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT) [![GitHub release (with filter)](https://img.shields.io/github/v/release/melon-masou/MonMouse)](https://github.com/melon-masou/MonMouse/releases)

The initial motivation behind MonMouse was reducing mouse movement when primarily working on the primary monitor and 
occasionally utilizing a pen tablet on the secondary monitor. The need to repeatedly move the mouse back to the primary monitor when switching to writing is tedious.

Additionally, MonMouse integrates features aimed at enhancing mouse workflow across multiple monitors. 

Glad to hear new ideas, feature requests, and feedback, which you can share on [Github Issues page](https://github.com/melon-masou/MonMouse/issues)!

**NOTE: MonMouse is different from mouse multiplexer designed for multiple users "sharing" one PC with their own mouse. Instead, it remembers the cursor's position when you switch to another mouse, eliminating the need to reposition the cursor each time. Please note that MonMouse is not intended for use with two mouses simultaneously.**

## Features

- Switch: Remembers the cursor position when moving to another mouse, and restores it when switching back.
- Lock: Resitrict the mouse cursor inside one monitor(includes a shortcut to jump between monitors).
- Partial support for various common pointer devices: mouse, touchpad, touchscreen, pen tablet, etc.
- No additional driver installation required.
- Currently only support Windows


## Usage
- Download prebuilt from [Releases](https://github.com/melon-masou/MonMouse/releases).
- MSVC library is required. You can get latest from [latest-supported-vc-redist](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)
- Only turn on "Switch" for **relative** positioning device which you hope to remember the cursor position, like mouse or touchpad. There is no need to turn on it for absolute positioning device like touchscreen and pen tablet.
- Some devices have multiple rows on "Devices" panel, which are corresponding to different usages. For example, changing "Pen Mode", "Mouse Mode", "Enable Windows Ink" setting of a Wacom tablet will cause mouse events emitted by different device.
- Some mouse events cannot be associated to any device, since Windows do not provide handle infomation. Defaultly MonMouse will associate them to the device which emits latest RAWINPUT event if it is within a brief period of time. Other events are associated to a dummy device "UnassiciatedEventsCapture". 
  - If you find two rows including "UnassiciatedEventsCapture" show "active" in device panel, even though you are moving just one mouse. You can increase this period by changing value of "Merge unassociated events within next" in config panel. 
  - The merging feature can be totally disabled by setting the value as -1.

## Build
```
# Debug
cargo build

# Release
# - windows Resource Compiler should be found at PATH or set RC_EXEC
cargo build --release
```

## License
This project is licensed under the MIT License.

## Screenshot
<img src="assets/screenshot1.png" width="50%">