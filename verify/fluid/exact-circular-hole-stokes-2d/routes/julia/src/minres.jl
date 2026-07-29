# Paige-Saunders MINRES with identity preconditioning, binary64.
#
# ADVISORY ONLY. This exists so the route can report whether the frozen solve
# selection is numerically feasible on this witness. It is a Julia analogue of
# that selection, not the registered `eqiora.reference` backend, and it is not a
# hosted measurement. It is never used to produce a frozen oracle value: every
# frozen number in this route comes from the elevated-precision solve.
#
# Validated in `audit.jl`'s companion path: on a well-conditioned system the
# recurred and independently reapplied true residuals agree to several digits.

using LinearAlgebra

"""
    minres(A, b; rtol, atol, maxiter, stop, probe)
        -> (x, iterations, recurred_residual, (best_true_residual, at_iteration))

Stops on the recurred residual estimate, exactly as a reference MINRES does.
The caller is responsible for independently reapplying the operator to obtain
the true residual.

`stop = false` disables the stopping test and `probe = k` reapplies the operator
every `k` iterations, so the caller can measure the true-residual floor that the
recurred estimate hides once Lanczos orthogonality is lost.
"""
function minres(A, b; rtol = 1e-11, atol = 1e-13, maxiter = 10000, stop = true,
                probe = 0)
    n = length(b)
    floor_true = Inf
    floor_at = 0
    x = zeros(n)
    beta1 = norm(b)
    beta1 == 0 && return x, 0, 0.0, (Inf, 0)
    r1 = copy(b); r2 = copy(b); y = copy(b)
    oldb = 0.0; beta = beta1; dbar = 0.0; epsln = 0.0
    phibar = beta1; cs = -1.0; sn = 0.0
    w = zeros(n); w2 = zeros(n)
    for itn in 1:maxiter
        v = y / beta
        y = A * v
        itn > 1 && (y .-= (beta / oldb) .* r1)
        alfa = dot(v, y)
        y .-= (alfa / beta) .* r2
        r1 = r2; r2 = y
        oldb = beta; beta = norm(y)
        oldeps = epsln
        delta = cs * dbar + sn * alfa
        gbar = sn * dbar - cs * alfa
        epsln = sn * beta
        dbar = -cs * beta
        gamma = max(sqrt(gbar^2 + beta^2), eps())
        cs = gbar / gamma; sn = beta / gamma
        phi = cs * phibar; phibar = sn * phibar
        w1 = w2; w2 = w
        w = (v - oldeps .* w1 - delta .* w2) ./ gamma
        x .+= phi .* w
        if probe > 0 && itn % probe == 0
            tr = norm(A * x - b)
            tr < floor_true && ((floor_true, floor_at) = (tr, itn))
        end
        stop && abs(phibar) <= max(atol, rtol * beta1) &&
            return x, itn, abs(phibar), (floor_true, floor_at)
    end
    x, maxiter, abs(phibar), (floor_true, floor_at)
end
