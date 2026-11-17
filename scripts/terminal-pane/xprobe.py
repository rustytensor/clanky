"""A small X11 client for speed measurements: key presses through XTest, drawing seen through the DAMAGE
extension, and screen pixels through XGetImage. It talks to the system's libX11, libXtst and libXdamage through
ctypes, so it needs no Python package beyond numpy.

Damage is watched on the root window: on a server without compositing every window draws into the root's
pixels, so one damage object sees every frame any client puts on screen.
"""

import ctypes
import os
import select
import time
from ctypes import POINTER, c_char_p, c_int, c_uint, c_ulong, c_void_p

import numpy as np

X_DAMAGE_NOTIFY = 0
X_DAMAGE_REPORT_NON_EMPTY = 3
Z_PIXMAP = 2
ALL_PLANES = c_ulong(0xFFFFFFFF)


class XImage(ctypes.Structure):
    pass


DESTROY_IMAGE = ctypes.CFUNCTYPE(c_int, POINTER(XImage))
XImage._fields_ = [
    ('width', c_int),
    ('height', c_int),
    ('xoffset', c_int),
    ('format', c_int),
    ('data', c_void_p),
    ('byte_order', c_int),
    ('bitmap_unit', c_int),
    ('bitmap_bit_order', c_int),
    ('bitmap_pad', c_int),
    ('depth', c_int),
    ('bytes_per_line', c_int),
    ('bits_per_pixel', c_int),
    ('red_mask', c_ulong),
    ('green_mask', c_ulong),
    ('blue_mask', c_ulong),
    ('obdata', c_void_p),
    ('create_image', c_void_p),
    ('destroy_image', DESTROY_IMAGE),
    ('get_pixel', c_void_p),
    ('put_pixel', c_void_p),
    ('sub_image', c_void_p),
    ('add_pixel', c_void_p),
]


def _load(name):
    try:
        return ctypes.CDLL(name)
    except OSError as error:
        raise RuntimeError(f'loading {name}: {error}; install the X11 client libraries') from error


_x11 = _load('libX11.so.6')
_xtst = _load('libXtst.so.6')
_damage = _load('libXdamage.so.1')

_x11.XOpenDisplay.restype = c_void_p
_x11.XOpenDisplay.argtypes = [c_char_p]
_x11.XCloseDisplay.argtypes = [c_void_p]
_x11.XDefaultRootWindow.restype = c_ulong
_x11.XDefaultRootWindow.argtypes = [c_void_p]
_x11.XConnectionNumber.argtypes = [c_void_p]
_x11.XPending.argtypes = [c_void_p]
_x11.XNextEvent.argtypes = [c_void_p, c_void_p]
_x11.XFlush.argtypes = [c_void_p]
_x11.XSync.argtypes = [c_void_p, c_int]
_x11.XStringToKeysym.restype = c_ulong
_x11.XStringToKeysym.argtypes = [c_char_p]
_x11.XKeysymToKeycode.restype = ctypes.c_ubyte
_x11.XKeysymToKeycode.argtypes = [c_void_p, c_ulong]
_x11.XGetImage.restype = POINTER(XImage)
_x11.XGetImage.argtypes = [c_void_p, c_ulong, c_int, c_int, c_uint, c_uint, c_ulong, c_int]
_xtst.XTestFakeKeyEvent.argtypes = [c_void_p, c_uint, c_int, c_ulong]
_damage.XDamageQueryExtension.argtypes = [c_void_p, POINTER(c_int), POINTER(c_int)]
_damage.XDamageCreate.restype = c_ulong
_damage.XDamageCreate.argtypes = [c_void_p, c_ulong, c_int]
_damage.XDamageSubtract.argtypes = [c_void_p, c_ulong, c_ulong, c_ulong]


class Display:
    """One connection to the X server named by `display` (":101")."""

    def __init__(self, display):
        self.handle = _x11.XOpenDisplay(display.encode())
        if not self.handle:
            raise RuntimeError(f'cannot open X display {display}')
        self.root = _x11.XDefaultRootWindow(self.handle)
        event_base, error_base = c_int(), c_int()
        if not _damage.XDamageQueryExtension(self.handle, ctypes.byref(event_base), ctypes.byref(error_base)):
            raise RuntimeError(f'X display {display} has no DAMAGE extension')
        self.damage_event = event_base.value + X_DAMAGE_NOTIFY
        self.damage = _damage.XDamageCreate(self.handle, self.root, X_DAMAGE_REPORT_NON_EMPTY)
        self.fd = _x11.XConnectionNumber(self.handle)
        self._event = ctypes.create_string_buffer(192)  # sizeof(XEvent) on 64-bit
        _x11.XSync(self.handle, 0)

    def close(self):
        if self.handle:
            _x11.XCloseDisplay(self.handle)
            self.handle = None

    def _drain(self):
        """Reads queued events; returns how many were damage notifications, and re-arms the damage object."""
        count = 0
        while _x11.XPending(self.handle):
            _x11.XNextEvent(self.handle, self._event)
            if ctypes.c_int.from_buffer(self._event).value == self.damage_event:
                count += 1
        if count:
            _damage.XDamageSubtract(self.handle, self.damage, 0, 0)
            _x11.XFlush(self.handle)
        return count

    def wait_damage(self, timeout):
        """Waits up to `timeout` seconds for drawing; returns the monotonic time it was seen, or None."""
        end = time.monotonic() + timeout
        while True:
            if self._drain():
                return time.monotonic()
            left = end - time.monotonic()
            if left <= 0:
                return None
            select.select([self.fd], [], [], left)

    def count_damage(self, seconds):
        """Counts damage notifications (one per burst of drawing) over `seconds`."""
        self._drain()
        end, count = time.monotonic() + seconds, 0
        while (left := end - time.monotonic()) > 0:
            select.select([self.fd], [], [], left)
            count += self._drain()
        return count

    def key(self, name):
        """Presses and releases one key through XTest; returns the monotonic time the press was flushed."""
        keycode = _x11.XKeysymToKeycode(self.handle, _x11.XStringToKeysym(name.encode()))
        if not keycode:
            raise RuntimeError(f'no keycode for key {name!r}')
        _xtst.XTestFakeKeyEvent(self.handle, keycode, 1, 0)
        _x11.XFlush(self.handle)
        pressed = time.monotonic()
        _xtst.XTestFakeKeyEvent(self.handle, keycode, 0, 0)
        _x11.XFlush(self.handle)
        return pressed

    def chord(self, *names):
        """Presses the first keys as modifiers, then the last key, and releases them all, through XTest; returns the
        monotonic time the last key's press was flushed."""
        codes = []
        for name in names:
            keycode = _x11.XKeysymToKeycode(self.handle, _x11.XStringToKeysym(name.encode()))
            if not keycode:
                raise RuntimeError(f'no keycode for key {name!r}')
            codes.append(keycode)
        for keycode in codes[:-1]:
            _xtst.XTestFakeKeyEvent(self.handle, keycode, 1, 0)
        _xtst.XTestFakeKeyEvent(self.handle, codes[-1], 1, 0)
        _x11.XFlush(self.handle)
        pressed = time.monotonic()
        for keycode in reversed(codes):
            _xtst.XTestFakeKeyEvent(self.handle, keycode, 0, 0)
        _x11.XFlush(self.handle)
        return pressed

    def grab(self, x, y, width, height):
        """Returns the screen pixels in the box as an (height, width, 4) uint8 array."""
        image = _x11.XGetImage(self.handle, self.root, x, y, width, height, ALL_PLANES, Z_PIXMAP)
        if not image:
            raise RuntimeError(f'XGetImage failed for {width}x{height}+{x}+{y}')
        try:
            info = image.contents
            if info.bits_per_pixel != 32:
                raise RuntimeError(f'expected 32 bits per pixel, the server gave {info.bits_per_pixel}')
            raw = ctypes.string_at(info.data, info.bytes_per_line * height)
            rows = np.frombuffer(raw, np.uint8).reshape(height, info.bytes_per_line)
            return rows[:, : width * 4].reshape(height, width, 4).copy()
        finally:
            image.contents.destroy_image(image)


def free_display():
    # The X server's lock files and sockets live at these fixed paths.
    for number in range(100, 200):
        if not os.path.exists(f'/tmp/.X{number}-lock') and not os.path.lexists(f'/tmp/.X11-unix/X{number}'):  # noqa: S108
            return f':{number}'
    raise RuntimeError('no free X display between :100 and :199')
