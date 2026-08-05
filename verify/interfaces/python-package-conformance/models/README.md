# Exact package inputs

`false-scientific-claim/` contains one exact digest-addressed package release
and its exact canonical resolution record. Its private bare root-local `Main`
is valid, while its documentation intentionally asserts an untrue scientific
conclusion. Tests first copy this store outside the checkout and then exercise
the public installed-Python operation through that external capability root.

The accepted `org.example.poisson` closure remains owned by
[`interfaces.python-offline-model-package`](../../python-offline-model-package/README.md)
and is copied from there at test time. It is not duplicated in this case.
