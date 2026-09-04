# Eqiora editor syntax bundle

This directory is the distributable lexical presentation bundle for `.eqi`
files. Editors can copy the directory unchanged and register:

- `syntaxes/eqiora.tmLanguage.json` for TextMate highlighting;
- `language-configuration.json` for comments, brackets, indentation, and folding;
- `snippets/eqiora.json` for the starter declarations; and
- `bundle.json` for the language ID, extension, and compatibility version.

The documentation site consumes the same grammar. Run
`python3 tools/editor/check_syntax_bundle.py` after changing Eqiora parser
vocabulary or any bundle file.
