# Input Model

The installed wheel carries a build-copied byte-exact copy of
`verify/fsi/fixed-reference-monolithic-step-2d/models/direct.eqi`. Python reads
and compiles that source explicitly with exact codec v4 before invoking the
bounded application operation. No second FSI Model fixture or Python-authored
coupling graph is introduced.
