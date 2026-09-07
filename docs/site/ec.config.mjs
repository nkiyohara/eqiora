import eqioraLanguage from '../../editor/eqiora/syntaxes/eqiora.tmLanguage.json' with { type: 'json' };
import { codeRegionAccessibilityPlugin } from './src/plugins/code-region-accessibility.ts';

export default {
  plugins: [codeRegionAccessibilityPlugin],
  shiki: {
    langs: [eqioraLanguage],
    langAlias: { eqi: 'eqiora' },
  },
};
