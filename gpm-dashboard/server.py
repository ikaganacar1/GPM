#!/usr/bin/env python3
"""
Simple reverse proxy server for GPM dashboard.
Serves frontend static files and proxies /api requests to backend.
"""

from http.server import HTTPServer, SimpleHTTPRequestHandler
import urllib.request
import urllib.error
import os
import sys

# Change to the dist directory
os.chdir('/mnt/2tb_ssd/GPM/gpm-dashboard/dist')

class ProxyHTTPRequestHandler(SimpleHTTPRequestHandler):
    API_BASE = "http://127.0.0.1:8010"

    def add_cors_headers(self):
        """Add CORS headers for cross-origin requests"""
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type')

    def do_OPTIONS(self):
        """Handle OPTIONS preflight requests"""
        self.send_response(200)
        self.add_cors_headers()
        self.end_headers()

    def do_GET(self):
        if self.path.startswith('/api/'):
            # Proxy API requests to backend
            api_url = f"{self.API_BASE}{self.path}"

            try:
                with urllib.request.urlopen(api_url) as response:
                    self.send_response(response.status)
                    self.add_cors_headers()
                    # Copy headers
                    for header, value in response.headers.items():
                        if header.lower() not in ('connection', 'transfer-encoding', 'access-control-allow-origin'):
                            self.send_header(header, value)
                    self.end_headers()
                    # Copy body
                    self.wfile.write(response.read())
            except urllib.error.HTTPError as e:
                self.send_response(e.code)
                self.add_cors_headers()
                self.send_header('Content-Type', 'application/json')
                self.end_headers()
                self.wfile.write(e.read().encode() if e.read() else b'{"error": "API request failed"}')
            except Exception as e:
                self.send_response(502)
                self.add_cors_headers()
                self.send_header('Content-Type', 'application/json')
                self.end_headers()
                self.wfile.write(f'{{"error": "{str(e)}"}}'.encode())
        else:
            # Serve static files (let parent handle it)
            SimpleHTTPRequestHandler.do_GET(self)

    def end_headers(self):
        # Disable caching for API requests
        if self.path.startswith('/api/'):
            self.send_header('Cache-Control', 'no-cache, no-store, must-revalidate')
            self.send_header('Pragma', 'no-cache')
            self.send_header('Expires', '0')
        super().end_headers()

    def log_message(self, format, *args):
        # Custom logging
        print(f"[{self.log_date_time_string()}] {format % args}")

def run_server(port=8011):
    server_address = ('', port)
    httpd = HTTPServer(server_address, ProxyHTTPRequestHandler)
    print(f"GPM Dashboard running on http://localhost:{port}")
    print(f"  Frontend: static files from dist/")
    print(f"  API proxy: -> http://127.0.0.1:8010")
    print(f"\nUse this port in your Cloudflare tunnel: {port}")
    print(f"  dummy.ikaganacar.com/* -> localhost:{port}")
    print()
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down...")
        httpd.server_close()

if __name__ == '__main__':
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8011
    run_server(port)
