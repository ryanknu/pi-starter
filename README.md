# Raspberry Pi Touch Starter

This project gets you started with everything you need to develop apps for the Raspberry Pi with 7-inch touchscreen.

It is also, mostly, educational for me. If you use this to start a project, please ensure that you can make it run
without modifications before doing anything. 

## What I use it for

My use case is probably not unique, but is likely not very common. I would like to use a Raspberry Pi with its 
touchscreen to develop apps for my children. I would prefer the device to have no other software on it other than these
apps. The kinds of apps I would like to have:

- Drawing with simple color picker, eraser, screen clear, and fun brushes.
- Swiping through read-only collection of family photos from my Immich instance.
- Playing simple games such as memory, minesweeper, snake, etc.
- Dashboards for a few Home Assistant controls.

It is in my use cases for the device to have network connectivity, but, a lack of other software on the device really
prevents misuse.

## Project Goals

My goal was to reduce dependencies to the bare minimum, to give the user basically the full power of the device as well
as an experiment in what you could do while minimizing external dependencies.

It should run perfectly smoothly on a model 3A+ and a model 4B. I have not tested other models.

## Provided Functionality

- An `InputDevice` concept that reads from Linux evdev events. This can easily be extended using `Touchscreen` as an
  example to handle mouse and keyboard, if desired.
- A `Touchscreen` concept that builds upon `InputDevice` that will let you basically write any kind of touch-first app.
- Utility function to detect the touchscreen hardware and provide the correct input event stream.
- A `Colorful` trait that lets you build colors and color brushes.
- A `ColorfulCycle` trait that takes an infinite `Iterator` and returns a `Colorful` for use with the `Screen`.
- A `Screen` concept that is a framebuffer with all the drawing primitives you would need to get things done, as well as
  coerces any `Colorful` objects into the screen's current bit depth. Setting pixels, drawing lines, rounded-corner and
  bordered rectangles, drawing images, and rendering text are all included.
- `hide_cursor` function, to stop the blinking cursor from the TTY.
- An example image pipeline that converts any assets in any format to RGBA bitmap in the compiled binary which can then
  be rendered by the `Screen` in its current bit depth. You can also forego this and simply use the `image` crate in
  your app.

## Future Functionality

- Read push button switches attached to GPIO
- Read attached USB webcam data

## Development Workflow

OpenSans is not shipped with this repository! Download from [here](http://www.opensans.com/download/open-sans-condensed.zip)
and `OpenSans-CondLight.ttf` to the `src/` directory.

Writing to the SD card can be very painful, even with rsync. Because of that, I recommend writing to /dev/shm. You can
use the following command to build and push:

```shell
cross build --target armv7-unknown-linux-musleabihf --release && rsync -rP target/armv7-unknown-linux-musleabihf/release user@host:/dev/shm
```

Apps have gotten bigger as I've added dependencies. You can strip out what you don't need:

- `rusttype` if you don't need to render fonts, or would prefer to use a bitmap font
- `image` if you don't need to render common image formats.

Run the built software via the terminal in Raspberry Pi OS Lite.

If your app gets "stuck" open, switch to tty2 (Ctrl+Alt+F2) and kill the app `kill -9 $(ps -ef | grep rpi | awk '{print $2}')`.
