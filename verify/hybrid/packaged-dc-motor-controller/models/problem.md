# Independent sampled-drive reference

With the held controller voltage (u_k), motor current (i), and angular
speed (omega), the package equations reduce independently to

\[
\frac{d}{dt}
\begin{bmatrix} i \\ \omega \end{bmatrix}
=
\begin{bmatrix}
-R/L & -k/L \\
k/J & -b/J
\end{bmatrix}
\begin{bmatrix} i \\ \omega \end{bmatrix}
+
\begin{bmatrix}1/L \\ 0\end{bmatrix}u_k.
\]

For this case,

\[
A=\begin{bmatrix}-4&-0.2\\2&-2\end{bmatrix},
\qquad B=\begin{bmatrix}2\\0\end{bmatrix}.
\]

The test evaluates each held interval without a numerical integrator:

\[
x(t+h)=x_{\mathrm{eq}}+e^{Ah}(x(t)-x_{\mathrm{eq}}),
\qquad x_{\mathrm{eq}}=-A^{-1}Bu_k.
\]

For a real (2\times2) matrix, write (A=sI+D), where
(s=\operatorname{tr}(A)/2=-3) and (D^2=0.6I). Then

\[
e^{Ah}=e^{sh}\left[
\cosh(\sqrt{0.6}h)I+
\frac{\sinh(\sqrt{0.6}h)}{\sqrt{0.6}}D
\right].
\]

At every exact 10 ms boundary, after advancing the plant state, the sampled
controller commits

\[
u_{k+1}=K_p(\omega_{\mathrm{set}}-\omega_{k+1}).
\]

This reference shares the physical parameters and tick contract, but it does
not call Eqiora's expression evaluator, junction composer, Newton solver, or
backward-Euler stepper.

For an accepted backward-Euler step, the combined discrete energy identity is

\[
ui = Ri^2+b\omega^2+\frac{E_n-E_{n-1}}{h}
+\frac{L(\Delta i)^2+J(\Delta\omega)^2}{2h},
\]

where (E=(Li^2+J\omega^2)/2). The last nonnegative term is backward-Euler
numerical dissipation and is reported separately from physical losses.
