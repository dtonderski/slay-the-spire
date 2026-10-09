"""Opt-in tailnet binding retains exact Host and same-origin protection."""
import tempfile
import unittest
from pathlib import Path

from fastapi.testclient import TestClient
from combat_explorer.server import AppConfig, create_app, safe_bind_host, valid_allowed_host


class NetworkTests(unittest.TestCase):
    def test_bind_and_hostname_validation(self):
        for host in ('localhost', '127.0.0.1', '100.94.109.85'):
            self.assertTrue(safe_bind_host(host))
        for host in ('0.0.0.0', '192.168.1.1', '8.8.8.8', '::', 'example.com'):
            self.assertFalse(safe_bind_host(host))
        self.assertTrue(valid_allowed_host('sorry.tail76d105.ts.net'))
        for host in ('', '*.ts.net', 'https://sorry.ts.net', 'sorry.ts.net:8765', 'sorry.ts.net/path'):
            self.assertFalse(valid_allowed_host(host))

    def test_tailnet_hosts_and_origins(self):
        hosts = ('100.94.109.85', 'sorry.tail76d105.ts.net')
        with tempfile.TemporaryDirectory() as directory:
            app = create_app(AppConfig(sessions_dir=Path(directory), host=hosts[0], allowed_hosts=hosts))
            with TestClient(app) as client:
                for host in hosts:
                    authority = f'{host}:8765'
                    self.assertEqual(client.get('/api/capabilities', headers={'host': authority, 'origin': f'http://{authority}'}).status_code, 200)
                    self.assertEqual(client.get('/api/capabilities', headers={'host': authority, 'origin': f'https://{authority}'}).status_code, 200)
                    self.assertEqual(client.get('/api/capabilities', headers={'host': authority, 'origin': f'http://{host}:9999'}).status_code, 403)
                    self.assertEqual(client.get('/api/capabilities', headers={'host': authority, 'origin': f'https://{host}'}).status_code, 403)
                self.assertEqual(client.get('/api/capabilities', headers={'host': 'evil.example:8765'}).status_code, 403)
                self.assertEqual(client.get('/api/capabilities', headers={'host': f'{hosts[0]}:8765', 'origin': f'https://{hosts[1]}:8765'}).status_code, 403)
