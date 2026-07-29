# Expected values

The non-implementing oracle is [`../oracle.py`](../oracle.py). It independently
freezes the complete 511-byte canonical JSON and the domain-separated SHA-256.
The Rust test executes it; no expected value is derived from production output.
