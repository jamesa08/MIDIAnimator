# the connection to MotionKeys (docs/bridge_protocol.md): one JSON message per line over a socket on this computer,
# {"id", "type", "reply_to", "body"}. the bridge connects, says hello, then answers MotionKeys' requests and sends the
# scene when it changes. MotionKeys never sends code to run, except as a run_script request

# using Singleton pattern found here
# https://refactoring.guru/design-patterns/singleton

import base64
import json
import os
import queue
import re
import socket
import threading
import uuid
import bpy
from bpy.app.handlers import persistent

# the protocol versions this bridge speaks, MotionKeys picks the newest one it speaks too
PROTOCOL_VERSIONS = [1]

# how long MotionKeys gets to answer the hello
HELLO_TIMEOUT = 5.0

# how often requests that came in are answered on the main thread, in seconds
ANSWER_INTERVAL = 0.02

ADDON_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def redraw_view3d():
    # the link panel lives in the 3D viewport sidebar
    for window in bpy.context.window_manager.windows:
        for area in window.screen.areas:
            if area.type == 'VIEW_3D':
                area.tag_redraw()
    # runs once
    return None


def bridge_version():
    """The add-on's version, from its manifest."""
    try:
        with open(os.path.join(ADDON_DIR, "blender_manifest.toml"), encoding="utf-8") as file:
            match = re.search(r'^version\s*=\s*"([^"]*)"', file.read(), re.MULTILINE)
        return match.group(1) if match else ""
    except OSError:
        return ""


def icon_data_url():
    """The icon MotionKeys shows for Blender, as a data URL. None when it's missing."""
    try:
        with open(os.path.join(ADDON_DIR, "icon.png"), "rb") as file:
            return "data:image/png;base64," + base64.b64encode(file.read()).decode("ascii")
    except OSError:
        return None


# the file name MotionKeys was last told, info goes out when it changes
_sent_file_name = None


def file_name():
    """The open file's name without its folder, empty for a file that was never saved."""
    return bpy.path.basename(bpy.data.filepath)


def hello_body():
    """Who the bridge is and what it can do, sent when it connects."""
    global _sent_file_name
    _sent_file_name = file_name()
    return {
        "protocol": PROTOCOL_VERSIONS,
        "app": "blender",
        "app_name": "Blender",
        "app_version": bpy.app.version_string,
        "bridge_version": bridge_version(),
        "file": _sent_file_name,
        "icon": icon_data_url(),
        "capabilities": {"watch": True, "scripts": ["python"]},
    }


class ServerMeta(type):
    _instances = {}

    def __call__(cls, *args, **kwargs):
        if cls not in cls._instances:
            instance = super().__call__(*args, **kwargs)
            cls._instances[cls] = instance
        return cls._instances[cls]


class Server(metaclass=ServerMeta):
    def __init__(self):
        # MotionKeys only listens on this computer
        self.host = '127.0.0.1'
        self.port = 6577
        self.socket = None
        self.connected = False
        # the protocol version MotionKeys picked in its hello
        self.protocol = None
        # the tracker sends from the main thread and the hello from the operator, a message is sent whole before
        # another one starts or their bytes mix
        self.send_lock = threading.Lock()
        # requests read on the receiving thread, answered on the main thread where bpy is safe to use
        self.requests = queue.Queue()
        # bytes read that aren't a whole message yet
        self._buffer = bytearray()
        self._scanned = 0

    def open(self):
        """Connects to MotionKeys and says hello. Raises with MotionKeys' reason when it turns the bridge away."""
        try:
            self.socket = socket.create_connection((self.host, self.port), timeout=HELLO_TIMEOUT)
            self._buffer = bytearray()
            self._scanned = 0
            # requests left from an earlier connection aren't answered on this one
            self.requests = queue.Queue()
            hello_id = self.send("hello", hello_body())
            reply = self._next_message()
            if reply.get("type") == "error":
                raise ConnectionRefusedError((reply.get("body") or {}).get("message") or "MotionKeys turned the bridge away")
            if reply.get("type") != "hello" or reply.get("reply_to") != hello_id:
                raise ConnectionError("MotionKeys didn't answer the hello")
            self.protocol = (reply.get("body") or {}).get("protocol")
            # from now on the receiving thread waits for messages as long as they take
            self.socket.settimeout(None)
        except Exception as e:
            # raised so the connect operator can show why
            print(f"Could not connect to MotionKeys at {self.host}:{self.port}: {e!r}")
            self.close()
            raise

        print(f"Connected to MotionKeys at {self.host}:{self.port}, protocol {self.protocol}")
        self.connected = True
        threading.Thread(target=self.receive_messages, daemon=True).start()
        if not bpy.app.timers.is_registered(answer_requests):
            bpy.app.timers.register(answer_requests, first_interval=0.0, persistent=True)

    def close(self):
        if self.socket:
            sock, self.socket = self.socket, None
            # wakes the receiving thread up from its read
            try:
                sock.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            sock.close()
            print("Disconnected from MotionKeys")
            self.connected = False
            self.protocol = None
            # the receiving thread closes this when MotionKeys quits, the panel only redraws on UI events and Blender's
            # UI is touched from the main thread, so a timer redraws it there
            bpy.app.timers.register(redraw_view3d)

    def send(self, type, body, reply_to=None):
        """Sends a message to MotionKeys, returns its id."""
        return self.send_raw(type, json.dumps(body), reply_to)

    def send_raw(self, type, body_json, reply_to=None):
        """Sends a message whose body is already JSON text, returns its id."""
        sock = self.socket
        if sock is None:
            raise ConnectionError("Not connected to MotionKeys")
        message_id = str(uuid.uuid4())
        data = f'{{"id": {json.dumps(message_id)}, "type": {json.dumps(type)}, "reply_to": {json.dumps(reply_to)}, "body": {body_json}}}\n'.encode("utf-8")
        # just the size, scene updates with keyframes can be megabytes and printing them stalls Blender
        print(f"MotionKeys sending {type} ({len(data)} bytes)")
        with self.send_lock:
            sock.sendall(data)
        return message_id

    def _next_message(self):
        """The next message from MotionKeys, waits until a whole line came in. Raises once the connection closes."""
        while True:
            end = self._buffer.find(b"\n", self._scanned)
            if end >= 0:
                line = bytes(self._buffer[:end])
                del self._buffer[:end + 1]
                self._scanned = 0
                if line.strip():
                    return json.loads(line)
                continue
            # a message of keyframes is megabytes, the part read already isn't searched again
            self._scanned = len(self._buffer)
            sock = self.socket
            if sock is None:
                raise ConnectionError("Not connected to MotionKeys")
            chunk = sock.recv(1 << 16)
            if not chunk:
                raise ConnectionError("MotionKeys closed the connection")
            self._buffer += chunk

    def receive_messages(self):
        """Reads MotionKeys' messages until the connection closes. Requests are answered on the main thread."""
        while self.socket:
            try:
                message = self._next_message()
            except Exception as e:
                if self.socket:
                    print("MotionKeys receiving error:", repr(e))
                    self.close()
                break

            print(f"MotionKeys received {message.get('type')}")
            # MotionKeys doesn't answer the bridge's messages after the hello, nothing waits for a reply
            if message.get("reply_to") is None:
                self.requests.put(message)


def answer_requests():
    """Answers the requests that came in, on Blender's main thread. Stops once the bridge is disconnected."""
    from . import handlers
    server = Server()
    while True:
        try:
            request = server.requests.get_nowait()
        except queue.Empty:
            break
        handlers.answer(server, request)
    if server.socket is None:
        return None
    return ANSWER_INTERVAL


@persistent
def send_file_name(*_):
    """Tells MotionKeys the open file's name once another file is opened or the file is saved under a new name."""
    global _sent_file_name
    server = Server()
    name = file_name()
    if not server.connected or name == _sent_file_name:
        return
    try:
        server.send("info", {"file": name})
        _sent_file_name = name
    except Exception as e:
        print(f"Failed to send the file name: {e}")


def register_file_handlers():
    for handlers in (bpy.app.handlers.load_post, bpy.app.handlers.save_post):
        if send_file_name not in handlers:
            handlers.append(send_file_name)


def unregister_file_handlers():
    for handlers in (bpy.app.handlers.load_post, bpy.app.handlers.save_post):
        if send_file_name in handlers:
            handlers.remove(send_file_name)
