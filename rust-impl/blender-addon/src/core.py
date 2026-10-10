# unorganized code for now, will be refactored later

# using Singleton pattern found here
# https://refactoring.guru/design-patterns/singleton


import socket
import select
import json
import threading
import types
import re
import traceback
import bpy

class ServerMeta(type):
    _instances = {}

    def __call__(cls, *args, **kwargs):
        if cls not in cls._instances:
            instance = super().__call__(*args, **kwargs)
            cls._instances[cls] = instance
        return cls._instances[cls]

class Server(metaclass=ServerMeta):
    def __init__(self):
        self.host = 'localhost'
        self.port = 6577
        self.socket = None
        self.connected = False
        # the tracker sends from the main thread and script results go out from the receiving thread, a message is
        # sent whole before another one starts or their bytes mix
        self.send_lock = threading.Lock()

    def open(self):
        try: 
            self.socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            self.socket.connect((self.host, self.port))
            print(f"Connected to server at {self.host}:{self.port}")

            # Start a separate thread to receive messages from the server
            threading.Thread(target=self.receive_messages, daemon=True).start()
            self.connected = True
        except Exception as e:
            # raised so the connect operator can show why
            print(f"Could not connect to server at {self.host}:{self.port}: {e!r}")
            self.close()
            raise

    def close(self):
        if self.socket:
            self.socket.close()
            self.socket = None
            print("Disconnected from server")
            self.connected = False

    def send_message(self, message, uuid):
        if not self.socket:
            raise Exception("Not connected to server")
        message_json = json.dumps({"sender": "client", "message": str(message), "uuid": uuid}) + '\n'
        # just the size, scene updates with keyframes can be megabytes and printing them stalls Blender
        print(f"MotionKeys sending {uuid} ({len(message_json)} bytes)")
        data = message_json.encode()
        
        # the socket doesn't block (receive_messages), so a full send buffer raises partway through a message instead of
        # waiting. MotionKeys reads it out bit by bit, a message of keyframes is megabytes, so wait until it can take more
        view = memoryview(data)
        with self.send_lock:
            while view:
                try:
                    view = view[self.socket.send(view):]
                except BlockingIOError:
                    select.select([], [self.socket], [], 1.0)


    def receive_messages(self):
        while self.socket:
            try:
                data = b''
                
                # turn off blocking, so we can manually check if we have the full message
                self.socket.setblocking(False)

                # need to loop over until we find the full `uuid` string with ending brace `}`. This is to ensure we have read the full message.
                # if the message itself contains a uuid message, this is not a valid message.
                while True:
                    try: 
                        chunk = self.socket.recv(1024)
                        if not chunk:
                            raise Exception("Server closed the connection")
                        
                        data += chunk

                    except socket.error as e:
                        # if the socket has no data available, ignore the exception
                        if e.errno != socket.errno.EWOULDBLOCK:
                            raise e
                        continue


                    # splits the string up by the end of the json object using regex (as we can use () to capture the split string in the result)
                    # then we filter out any empty strings from the split
                    
                    try:
                        check = list(filter(None, re.split('("})', data.decode("utf-8").strip())))
                        # two checks to see if we have the full message, if we have "uuid": and "} (don't particularly care about the rest of the uuid)
                        if len(check) >= 2 and check[-1] == '"}' and '"uuid":' in check[-2] or not chunk:
                            break
                    except Exception as e:
                        print("error", e)
                
                raw_message = json.loads(data.strip())
                # if raw_message["sender"] == "server":
                print(f"MotionKeys received {raw_message.get('uuid')} ({len(data)} bytes)")
                
                # if raw_message["message"][:3] == "":
                d = {"bpy": bpy}
                lv = {}
                try:
                    # exec(f"{raw_message['message']}", d, lv)
                    # # res_func = lv['execution']
                    # # res = res_func()
                    # functions = {name: obj for name, obj in lv.items() if callable(obj)}
                    # res = functions["execution"]()
                    # print(f"RES {res}")
                    
                    compiled_code = compile(raw_message['message'], '<string>', 'exec')  # compile the code string as a code object

                    ipc_runtime = types.ModuleType('ipc_runtime')  # create a new module object

                    ipc_runtime.__dict__['bpy'] = bpy  # add bpy to module

                    exec(compiled_code, ipc_runtime.__dict__) # execute the code object in the context of the new module

                    execute_func = ipc_runtime.execute   # retrieve the functions from the module

                    res = execute_func()

                    if res:
                        self.send_message(res, raw_message["uuid"])
                except Exception as e:
                    # the full traceback goes back, MotionKeys shows it on the node that asked
                    self.send_message(f"MotionKeys IPC execution error: {traceback.format_exc()}", raw_message["uuid"])
                    print("MotionKeys IPC execution error:", str(e))

            except Exception as e:
                print("MotionKeys IPC receiving error:", str(e))
                self.close()
                break