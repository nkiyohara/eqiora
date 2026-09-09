# Mathematical reference

Use the product rule `(fg)_x=f_x*g+f*g_x` and composition rule
`(f(g(x)))_x=f_g(g(x))*g_x`, holding distinct declared independent inputs fixed.
The [accepted language calculus contract](../../../../docs/language/calculus.md) owns
binding identity and result dimensions.

For `f=x*x*y`, direct Component arguments `(x=p,y=q)` retain the parent
directions: `f_p=2*p*q` and `f_q=p*p`. Swapping the arguments swaps those
identities even when `p=q=3`. Literal and derived arguments do not allocate a
new independent Parameter; binding both slots to one parent also makes
`holding=(y)` conflict with `wrt=x`.
