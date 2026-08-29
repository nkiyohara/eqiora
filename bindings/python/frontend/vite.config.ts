import { defineConfig } from "vite";

export default defineConfig({
	build: {
		lib: {
			entry: "src/index.ts",
			formats: ["es"],
			fileName: () => "viewer.mjs",
			cssFileName: "viewer",
		},
		outDir: "../python/eqiora/_viewer/static",
		emptyOutDir: true,
		sourcemap: false,
	},
});
