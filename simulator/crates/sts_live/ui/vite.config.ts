import { defineConfig } from "vite";

export default defineConfig({
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/bridges": "http://127.0.0.1:8800",
      "/sessions": "http://127.0.0.1:8800"
    }
  }
});
