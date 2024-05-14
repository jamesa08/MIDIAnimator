# unorganized code for now, will be refactored later

# using Singleton pattern found here
# https://refactoring.guru/design-patterns/singleton


import socket
import json
import threading
import types
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

    def open(self):
        try: 
            self.socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            self.socket.connect((self.host, self.port))
            print(f"Connected to server at {self.host}:{self.port}")

            # Start a separate thread to receive messages from the server
            threading.Thread(target=self.receive_messages, daemon=True).start()
            self.connected = True
            return True
        except: 
            print("Could not connect to server")
            self.close()
            return False

    def close(self):
        if self.socket:
            self.socket.close()
            self.socket = None
            print("Disconnected from server")
            self.connected = False

    def send_message(self, message, uuid):
        if not self.socket:
            raise Exception("Not connected to server")
        message_json = json.dumps({"sender": "client", "message": message, "uuid": uuid}) + '\n'
        print(message_json)
        self.socket.sendall(message_json.encode())

    def receive_messages(self):
        while self.socket:
            try:
                data = self.socket.recv(1024).decode()
                if not data:
                    raise Exception("Server closed the connection")
                
                print(data)
                raw_message = json.loads(data.strip())
                # if raw_message["sender"] == "server":
                print(raw_message)
                
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

                    # Compile the code string as a code object
                    compiled_code = compile(raw_message['message'], '<string>', 'exec')

                    # Create a new module object
                    ipc_runtime = types.ModuleType('ipc_runtime')

                    # add bpy to the module 
                    ipc_runtime.__dict__['bpy'] = bpy

                    # Execute the code object in the context of the new module
                    exec(compiled_code, ipc_runtime.__dict__)

                    # Retrieve the functions from the module
                    execution_func = ipc_runtime.execution

                    # Call the functions
                    res = execution_func()

                    if res:
                        self.send_message(res, raw_message["uuid"])
                except Exception as e:
                    self.send_message(f"MIDIAnimator IPC execution error: {str(e)}", raw_message["uuid"])
                    print("MIDIAnimator IPC execution error:", str(e))

            except Exception as e:
                print("MIDIAnimator IPC reciving error:", str(e))
                self.close()
                break