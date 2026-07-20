import socket
import sys

HOST = '127.0.0.1'
PORT = 8888

server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)

try:
    server.bind((HOST, PORT))
except Exception as e:
    print(f"Failed to bind to {HOST}:{PORT}: {e}")
    sys.exit(1)

server.listen(5)
print(f"TCP Socket Server (Short Connection Echo) listening on {HOST}:{PORT}...")

try:
    while True:
        conn, addr = server.accept()
        print(f"\n[+] Connected by {addr}")
        try:
            data = conn.recv(4096)
            if data:
                print(f"Received (Str): {data.decode('utf-8', errors='replace')}")
                print(f"Received (Hex): {data.hex(' ')}")
                # Echo back
                conn.sendall(data)
        except Exception as e:
            print(f"Error handling data: {e}")
        conn.close()
        print(f"[-] Connection closed for {addr}")
except KeyboardInterrupt:
    print("\nShutting down server.")
finally:
    server.close()
