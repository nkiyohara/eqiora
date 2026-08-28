import { existsSync, readFileSync } from 'node:fs';
import { stat, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { satteri } from '@astrojs/markdown-satteri';
import starlight from '@astrojs/starlight';
import { defineConfig } from 'astro/config';

import { codeRegionAccessibilityPlugin } from './src/plugins/code-region-accessibility.ts';
import { katexMathPlugin } from './src/plugins/katex.ts';

const SOURCE_SHA = /^[0-9a-f]{40}$/;
const eqioraLanguage = JSON.parse(
  readFileSync(fileURLToPath(new URL('./src/syntaxes/eqiora.tmLanguage.json', import.meta.url)), 'utf8'),
);

function outputDirectory() {
  const configured = process.env.EQIORA_SITE_ASTRO_OUT_DIR;
  if (configured) {
    return resolve(configured);
  }
  return fileURLToPath(new URL('../../build/site-astro/', import.meta.url));
}

const robots = {
  name: 'eqiora-static-robots',
  hooks: {
    'astro:build:done': async ({ dir }) => {
      const output = fileURLToPath(dir);
      const sitemap = resolve(output, 'sitemap-index.xml');
      if (!(await stat(sitemap)).isFile()) {
        throw new Error('Starlight sitemap-index.xml is absent from the static output');
      }
      await writeFile(
        resolve(output, 'robots.txt'),
        'User-agent: *\nAllow: /\nSitemap: https://eqiora.org/sitemap-index.xml\n',
        { encoding: 'utf8', flag: 'wx', mode: 0o644 },
      );
    },
  },
};

if (process.env.EQIORA_SITE_BUILD_PROFILE === 'complete') {
  if (!SOURCE_SHA.test(process.env.EQIORA_SITE_SOURCE_SHA ?? '')) {
    throw new Error('EQIORA_SITE_SOURCE_SHA must be the exact 40-character lowercase source commit');
  }

  const requiredSuccessorInputs = [
    'src/assets/brand/eqiora-mark.svg',
    'src/assets/gallery/exact-cylinder-pressure.png',
    'src/assets/gallery/mixed-boundary-elasticity-displacement.png',
    'src/components/site/ExactSourceLink.astro',
    'src/components/site/ReleaseIdentity.astro',
    'src/content/docs/index.mdx',
    'src/content/docs/evidence/index.mdx',
    'src/content/docs/gallery/index.mdx',
    'src/content/docs/gallery/exact-cylinder-steady-stokes.mdx',
    'src/content/docs/gallery/mixed-boundary-elasticity.mdx',
    'src/content/docs/reference/index.mdx',
    'src/content/docs/reference/python/index.mdx',
    'src/content/docs/reference/rust/index.mdx',
    'src/content/docs/reference/cli/index.mdx',
    'src/content/docs/reference/control-v2/index.mdx',
    'src/content/docs/reference/mcp/index.mdx',
    'src/data/gallery/exact-cylinder-steady-stokes.publication.json',
    'src/data/gallery/mixed-boundary-elasticity.publication.json',
    'src/styles/site/tokens.css',
    'src/styles/site/layout.css',
    'src/styles/site/components.css',
    'public/favicon.svg',
    'public/apple-touch-icon.png',
    'public/social-card.svg',
  ];
  const missing = requiredSuccessorInputs.filter(
    (relative) => !existsSync(fileURLToPath(new URL(relative, import.meta.url))),
  );
  if (missing.length > 0) {
    throw new Error(`successor-site dependencies are absent: ${missing.join(', ')}`);
  }
}

export default defineConfig({
  site: 'https://eqiora.org/',
  base: '/',
  output: 'static',
  outDir: outputDirectory(),
  trailingSlash: 'always',
  markdown: {
    processor: satteri({
      features: { math: true },
      mdastPlugins: [katexMathPlugin],
    }),
  },
  integrations: [
    starlight({
      title: 'Eqiora',
      description: 'Meaning-first scientific modeling and execution.',
      logo: {
        src: './src/assets/brand/eqiora-mark.svg',
        alt: '',
        replacesTitle: false,
      },
      favicon: '/favicon.svg',
      expressiveCode: {
        plugins: [codeRegionAccessibilityPlugin],
        shiki: {
          langs: [eqioraLanguage],
          langAlias: { eqi: 'eqiora' },
        },
      },
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/nkiyohara/eqiora',
        },
      ],
      sidebar: [
        { label: 'Docs', link: '/get-started/' },
        { label: 'Gallery', link: '/gallery/' },
        { label: 'Reference', link: '/reference/' },
        { label: 'Evidence', link: '/evidence/' },
      ],
      head: [
        { tag: 'meta', attrs: { property: 'og:type', content: 'website' } },
        {
          tag: 'meta',
          attrs: { property: 'og:image', content: 'https://eqiora.org/social-card.svg' },
        },
        {
          tag: 'meta',
          attrs: { name: 'twitter:card', content: 'summary_large_image' },
        },
        {
          tag: 'meta',
          attrs: { name: 'twitter:image', content: 'https://eqiora.org/social-card.svg' },
        },
        { tag: 'link', attrs: { rel: 'apple-touch-icon', href: '/apple-touch-icon.png' } },
      ],
      customCss: [
        'katex/dist/katex.min.css',
        '/src/styles/site/tokens.css',
        '/src/styles/site/layout.css',
        '/src/styles/site/components.css',
      ],
      components: {
        Search: './src/components/site/Search.astro',
        ThemeSelect: './src/components/site/ThemeSelect.astro',
      },
      pagefind: true,
      lastUpdated: false,
    }),
    robots,
  ],
  vite: {
    build: {
      sourcemap: false,
    },
  },
});
