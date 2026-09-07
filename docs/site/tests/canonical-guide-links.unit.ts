import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { markdownToHtml } from 'satteri';
import { canonicalGuideLinks } from '../src/plugins/canonical-guide-links.ts';

const sha = '1234567890abcdef1234567890abcdef12345678';
const sourceURL = (name: string) => new URL(`../../python/${name}.md`, import.meta.url);

test('each actual maintained body keeps its content and converts repository anchors', () => {
  process.env.EQIORA_SITE_SOURCE_SHA = sha;
  for (const name of ['modeling', 'execution-and-arrays', 'differentiation']) {
    const fileURL = sourceURL(name);
    const source = readFileSync(fileURL, 'utf8');
    const { html } = markdownToHtml(source, { fileURL, mdastPlugins: [canonicalGuideLinks] });
    assert.ok(!html.includes('<h1>'), name);
    assert.ok(html.includes('<h2>'), name);
    assert.ok(html.includes('<pre>'), name);
    assert.ok(!html.includes('href="../../examples/'), name);
    if (name === 'modeling') {
      for (const filename of ['steady_cylinder_source.py', 'exact_cylinder_stokes.py', 'mixed_boundary_elasticity.py', 'fixed_reference_fsi.py']) {
        assert.ok(html.includes(`href="https://github.com/nkiyohara/eqiora/blob/${sha}/examples/python/${filename}"`));
      }
    }
    if (name === 'differentiation') assert.ok(html.includes('href="/guides/execution-and-arrays/"'));
  }
});

test('local guide fragments and exact-current source links retain their destinations', () => {
  process.env.EQIORA_SITE_SOURCE_SHA = sha;
  const { html } = markdownToHtml('[guide](execution-and-arrays.md#structured-failures) [source](../../examples/decay.eqi)', {
    fileURL: sourceURL('modeling'), mdastPlugins: [canonicalGuideLinks],
  });
  assert.ok(html.includes('href="/guides/execution-and-arrays/#structured-failures"'));
  assert.ok(html.includes(`href="https://github.com/nkiyohara/eqiora/blob/${sha}/examples/decay.eqi"`));
});

test('the presentation adapter does not rewrite other Markdown or permit escaping source links', () => {
  process.env.EQIORA_SITE_SOURCE_SHA = sha;
  const outside = markdownToHtml('# Heading\n\n[guide](execution-and-arrays.md)', {
    fileURL: sourceURL('api'), mdastPlugins: [canonicalGuideLinks],
  }).html;
  assert.ok(outside.includes('<h1>Heading</h1>'));
  assert.ok(outside.includes('href="execution-and-arrays.md"'));
  assert.throws(() => markdownToHtml('[outside](../../../private.txt)', {
    fileURL: sourceURL('modeling'), mdastPlugins: [canonicalGuideLinks],
  }), /escapes the repository/);
});

test('source conversion requires an immutable source identity', () => {
  for (const value of ['', 'main', 'v0.1.0a7', sha.toUpperCase()]) {
    process.env.EQIORA_SITE_SOURCE_SHA = value;
    assert.throws(() => markdownToHtml('[source](../../examples/decay.eqi)', {
      fileURL: sourceURL('modeling'), mdastPlugins: [canonicalGuideLinks],
    }), /exact source SHA/);
  }
});
