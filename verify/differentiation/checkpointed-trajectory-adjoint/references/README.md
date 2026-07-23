# Reference

The independent map repeats

```text
x_next = x_previous / (1 + h p)
z_next = x_next^2
```

four times and evaluates `z_final + 0.125 p`. Centered differences with
`epsilon = 1e-6` provide the initial-state and Parameter gradient oracle.
