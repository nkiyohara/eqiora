import { defineConfig, type Plugin } from "vite";

function includeStyles(): Plugin {
	const entry = new URL("./src/mesh-view.ts", import.meta.url).pathname;
	return {
		name: "eqiora-view-styles",
		enforce: "pre",
		transform(source, id) {
			return id === entry
				? `import "./mesh-view.css";\nimport "./trajectory-view.css";\n${source}`
				: undefined;
		},
	};
}

export default defineConfig({
	plugins: [includeStyles()],
	build: {
		copyPublicDir: false,
		cssCodeSplit: false,
		emptyOutDir: true,
		lib: {
			entry: new URL("./src/mesh-view.ts", import.meta.url).pathname,
			formats: ["es"],
			fileName: () => "mesh-view.mjs",
			cssFileName: "mesh-view",
		},
		minify: "oxc",
		outDir: "dist",
		reportCompressedSize: false,
		rollupOptions: {
			external: [],
			output: {
				assetFileNames: (asset) =>
					asset.name === "style.css" ? "mesh-view.css" : "[name][extname]",
			},
		},
		sourcemap: false,
		target: "es2022",
	},
});
