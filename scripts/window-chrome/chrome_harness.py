"""The real-window harness for the window chrome gates: a private Xvfb, an isolated clanky-gpui, xdotool and the record.

Isolation is G1's: nothing reaches the user's desktop session (no Wayland socket, no session bus), the runtime folder
is exactly mode 0700, every folder is scratch, and the session server lives in the scratch folder. Windows and the
Xvfb are stopped by process ID. The simulated window (simulated_window.py) offers the same methods.
"""

import json
import os
import socket
import subprocess
import time
from pathlib import Path

import numpy as np
from PIL import Image

LOGICAL_W, LOGICAL_H = 1200, 800
VULKAN_ICD = "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json"
DISPLAY_FIRST, DISPLAY_LAST = 100, 199


class Fail(Exception):
    pass


def until(condition, label, timeout=5.0):
    end = time.monotonic() + timeout
    while True:
        value = condition()
        if value:
            return value
        if time.monotonic() > end:
            raise Fail(label)
        time.sleep(0.05)


def private_folder(path):
    """A folder of mode exactly 0700: folders under /srv/devops inherit the setgid bit, which the server refuses."""
    path.mkdir(parents=True, exist_ok=True)
    subprocess.run(["chmod", "g-s,u=rwx,go=", str(path)], check=True)


def free_display():
    """A display no X server uses, :100 and up (never :0, the user's desktop)."""
    for number in range(DISPLAY_FIRST, DISPLAY_LAST + 1):
        if not any(
            os.path.lexists(p)
            for p in (f"/tmp/.X{number}-lock", f"/tmp/.X11-unix/X{number}")
        ):
            return str(number)
    raise Fail(f"no free X display between :{DISPLAY_FIRST} and :{DISPLAY_LAST}")


def latest_landmarks(records):
    """{name: [box, ...]}: the last line written for each name and index, the boxes in index order."""
    latest = {}
    for line in records:
        if line.get("record") == "landmark":
            latest[(line["name"], line.get("index", 0))] = line["box"]
    result = {}
    for (name, index), box in sorted(latest.items()):
        result.setdefault(name, []).append(box)
    return result


class Conn:
    """The session server's owner API, for pane text."""

    def __init__(self, path):
        self.stream = socket.socket(socket.AF_UNIX)
        self.stream.settimeout(30)
        self.stream.connect(str(path))
        self.reader = self.stream.makefile("rb")
        self.seq = 0

    def call(self, version, op, **arguments):
        self.seq += 1
        message = dict(v=version, id=str(self.seq), op=op, **arguments)
        self.stream.sendall((json.dumps(message) + "\n").encode())
        reply = json.loads(self.reader.readline(4 << 20))
        if reply.get("ok") is not True:
            raise Fail(f"RPC {op}: {reply.get('error')}")
        return reply["result"]


class RealWindow:
    """One clanky-gpui window at one scale, on its own Xvfb."""

    kind = "real"

    def __init__(self, scale, work, app, config, fault=None, keep=False):
        self.scale, self.app, self.config, self.fault, self.keep = (
            scale,
            app,
            config,
            fault,
            keep,
        )
        self.pw, self.ph = round(LOGICAL_W * scale), round(LOGICAL_H * scale)
        self.work = work / f"scale-{scale:g}"
        for name in ("home", "config", "data", "state", "runtime", "cache"):
            private_folder(self.work / name)
        private_folder(self.work)
        # The tab is named after the folder the shell starts in: "HIH", capitals with flat tops, for the cap height check.
        (self.work / "home/HIH").mkdir(exist_ok=True)
        (self.work / "home/.bashrc").write_text("PS1='$ '\ncd ~/HIH\n")
        self.record_path = self.work / "controls.jsonl"
        self.procs, self.server = [], None
        try:
            self._start()
        except BaseException:
            self.close()
            raise

    def _start(self):
        env = {
            k: v
            for k, v in os.environ.items()
            if not k.startswith(
                (
                    "XDG_",
                    "CLANKY",
                    "WAYLAND",
                    "DBUS",
                    "GNOME",
                    "GSETTINGS",
                    "VTE",
                    "DISPLAY",
                )
            )
        }
        env.update(
            HOME=str(self.work / "home"),
            XDG_CONFIG_HOME=str(self.work / "config"),
            XDG_DATA_HOME=str(self.work / "data"),
            XDG_STATE_HOME=str(self.work / "state"),
            XDG_CACHE_HOME=str(self.work / "cache"),
            XDG_RUNTIME_DIR=str(self.work / "runtime"),
            SHELL="/bin/bash",
            TZ="UTC",
            GSETTINGS_BACKEND="memory",
            GPUI_X11_SCALE_FACTOR=f"{self.scale:g}",
            LP_NUM_THREADS="8",
            VK_DRIVER_FILES=VULKAN_ICD,
            CLANKY_CONTROL_RECORD=str(self.record_path),
            CLANKY_FRAME_TRACE=str(self.work / "frames.csv"),
        )
        if self.fault:
            env["CLANKY_CONTROL_FAULT"] = self.fault
        config = self.work / "config.json"
        config.write_text(json.dumps(self.config), encoding="utf-8")
        self.config_path = (
            config  # the file the window edits: the Settings round trip reads it
        )
        read_end, write_end = os.pipe()
        xvfb = subprocess.Popen(
            [
                "Xvfb",
                ":" + free_display(),
                "-displayfd",
                str(write_end),
                "-screen",
                "0",
                f"{self.pw + 200}x{self.ph + 200}x24",
                "-nolisten",
                "tcp",
            ],
            pass_fds=(write_end,),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        self.procs.append(xvfb)
        os.close(write_end)
        env["DISPLAY"] = ":" + os.read(read_end, 32).decode().strip()
        os.close(read_end)
        self.env = env
        sock = self.work / "window.sock"
        error_file = self.work / "window.err"
        window = subprocess.Popen(
            [
                str(self.app),
                "--socket",
                str(sock),
                "--config",
                str(config),
                "--size",
                f"{LOGICAL_W}x{LOGICAL_H}",
            ],
            env=env,
            cwd=str(self.work / "home"),
            stdout=subprocess.DEVNULL,
            stderr=error_file.open("wb"),
        )
        self.procs.append(window)

        def socket_ready():
            if window.poll() is not None:
                raise Fail(
                    f"window exited: {error_file.read_text(errors='replace').strip()[-300:]}"
                )
            return sock.exists()

        until(socket_ready, "window socket", 40)
        self.wid = until(
            lambda: (
                subprocess.run(
                    ["xdotool", "search", "--pid", str(window.pid)],
                    env=env,
                    capture_output=True,
                    text=True,
                ).stdout.split()
                or [None]
            )[0],
            "window",
            40,
        )
        self.x("windowsize", "--sync", self.wid, str(self.pw), str(self.ph))
        self.x("windowmove", "--sync", self.wid, "0", "0")
        self.owner = Conn(sock)
        self.owner.call(3, "auth", token=Path(str(sock) + ".owner").read_text().strip())
        self.server = self.owner.call(3, "hello").get("server_pid")
        until(lambda: self.owner.call(1, "list_tabs")["tabs"], "first pane", 20)
        time.sleep(2)
        self.x("windowfocus", "--sync", self.wid)
        # A second tab, so the tab-switching checks have an inactive tab; the first stays active.
        self.key("ctrl+shift+t")
        until(
            lambda: len(self.owner.call(1, "list_tabs")["tabs"]) >= 2, "second tab", 20
        )
        self.key("ctrl+Tab")
        time.sleep(1)
        self.away()

    def menu_point(self):
        """A point inside the first pane, left of the middle and in the upper half, with room for the menu."""
        return int(0.45 * self.pw), int(0.3 * self.ph)

    def x(self, *args):
        subprocess.run(
            ["xdotool", *args], env=self.env, check=True, stdout=subprocess.DEVNULL
        )

    def key(self, *keys):
        for name in keys:
            self.x("key", name)
            time.sleep(0.15)

    def type(self, text):
        self.x("type", "--delay", "25", text)

    def move(self, x, y):
        self.x("mousemove", "--window", self.wid, str(int(x)), str(int(y)))

    def away(self, point=None):
        x, y = point or (int(0.9 * self.pw), int(0.5 * self.ph))
        self.move(x, y)
        time.sleep(0.25)

    def shot(self, name):
        path = self.work / f"{name}.png"
        subprocess.run(
            [
                "ffmpeg",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "x11grab",
                "-draw_mouse",
                "0",
                "-video_size",
                f"{self.pw}x{self.ph}",
                "-i",
                self.env["DISPLAY"] + "+0,0",
                "-frames:v",
                "1",
                str(path),
            ],
            env=self.env,
            check=True,
            stdin=subprocess.DEVNULL,
        )
        image = np.asarray(Image.open(path).convert("RGB")).astype(int)
        if not self.keep:
            path.unlink()
        return image

    def records(self):
        lines = []
        if self.record_path.exists():
            for line in self.record_path.read_text().splitlines():
                try:
                    lines.append(json.loads(line))
                except ValueError:
                    continue  # the writer is still on this line
        return lines

    def screen(self, name):
        found = None
        for line in self.records():
            if line.get("record") == "screen" and line.get("screen") == name:
                found = line
        return found

    def activations(self, name):
        return [
            r
            for r in self.records()
            if r.get("record") == "activate" and r.get("screen") == name
        ]

    def landmarks(self):
        return latest_landmarks(self.records())

    def frame_count(self):
        """Frames the window has drawn, from its frame trace (one "frame,..." line each)."""
        trace = self.work / "frames.csv"
        if not trace.exists():
            return 0
        return sum(line.startswith("frame,") for line in trace.read_text().splitlines())

    def first_pane(self):
        """The id of the first pane, from the owner API. TODO(G2): the shape of list_tabs' answer is a guess."""
        tabs = self.owner.call(1, "list_tabs")["tabs"]
        first = tabs[0]
        return (
            first.get("tab_id", first.get("id")) if isinstance(first, dict) else first
        )

    def pane_text(self, pane):
        screen = self.owner.call(
            4, "screen", tab_id=pane, history_offset=0, history_limit=64
        )
        lines = screen.get("history", {}).get("lines", []) + screen.get("lines", [])
        return "\n".join(
            line if isinstance(line, str) else line.get("text", "") for line in lines
        )

    def text_count(self, text):
        """How many times `text` stands in the pane that holds it most, from the server's copy of every pane's text
        (history and screen), independent of the window's find bar."""
        tabs = self.owner.call(1, "list_tabs")["tabs"]
        ids = [t.get("tab_id", t.get("id")) if isinstance(t, dict) else t for t in tabs]
        return max((self.pane_text(pane).count(text) for pane in ids), default=0)

    def close(self):
        if self.server:
            try:
                os.kill(self.server, 15)
            except ProcessLookupError:
                pass
        for process in reversed(self.procs):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
