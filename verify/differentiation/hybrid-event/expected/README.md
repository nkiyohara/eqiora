# Expected evidence

The Rust integration test is the executable analytic oracle. It checks the
complete matrices and vectors rather than a rounded golden table, so exact
Parameter/state ordering and every derivative entry remain part of the test.
