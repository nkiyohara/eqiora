"""Explicit static support selections shared by Python consumer fixtures."""


def support_bindings(geometry, regions=(), boundaries=()):
    result = {name: geometry.selection(name) for name in regions}
    result.update({
        name: (geometry.selection(name), geometry.selection(parent))
        for name, parent in boundaries
    })
    return result
