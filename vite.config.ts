import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  // NAURIX_DEV_PORT lets a second, isolated dev instance run next to your own (see README).
  server: { port: Number(process.env.NAURIX_DEV_PORT ?? 1420), strictPort: true },
  build: { target: "safari15" },
});
